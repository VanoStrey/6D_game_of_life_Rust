//! Hardened Simulation Worker running Game of Life generations in a background thread.
//!
//! Provides decoupled, bounded, thread-safe communication between the
//! simulation engine and the UI / renderer with strict backpressure,
//! non-blocking command dispatch, guaranteed monotonic generation ordering,
//! and epoch versioning.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossbeam_channel::{bounded, Receiver, RecvTimeoutError, Sender, TrySendError};
use rand::rngs::StdRng;
use rand::SeedableRng;

use crate::projection::{collect_alive_positions, VisualBounds};
use crate::simulation::{Grid, Rules, Simulation};

/// Complete simulation state snapshot exported from the background worker.
#[derive(Debug, Clone)]
pub struct ExportedSimulationState {
    pub generation: u64,
    pub dimensions: usize,
    pub size: usize,
    pub delta: usize,
    pub percent_min: f64,
    pub percent_max: f64,
    pub periodic: bool,
    pub total_cells: usize,
    pub alive_count: usize,
    pub grid: Grid,
}

/// Commands sent from the UI/render thread to the simulation worker.
#[derive(Debug, Clone)]
pub enum SimulationCommand {
    /// Compute exactly one simulation step.
    Step,
    /// Enable or disable continuous playback.
    SetPlaying(bool),
    /// Set generation target speed (generations per second).
    SetSpeed(f32),
    /// Re-randomize the grid with an optional seed and reset generation to 0.
    Randomize { epoch: u64, seed: Option<u64> },
    /// Reconfigure the hypergrid with new dimensions, size, rules, topology and epoch.
    Reconfigure {
        epoch: u64,
        size: usize,
        dimensions: usize,
        delta: usize,
        percent_min: f64,
        percent_max: f64,
        periodic: bool,
        seed: Option<u64>,
    },
    /// Export the full current simulation state (grid, rules, generation, etc.).
    ExportState {
        reply_tx: Sender<Box<ExportedSimulationState>>,
    },
    /// Load a full simulation state into the worker thread.
    LoadState {
        epoch: u64,
        delta: usize,
        rules: Rules,
        periodic: bool,
        generation: u64,
        grid: Grid,
    },
    /// Shutdown the background worker thread cleanly.
    Shutdown,
}

/// An immutable generation snapshot produced by the simulation worker.
#[derive(Debug, Clone)]
pub struct SimulationSnapshot {
    /// Configuration epoch / version to detect and discard stale updates.
    pub epoch: u64,
    /// Generation number (0 = initial state).
    pub generation: u64,
    /// Time spent computing this generation in milliseconds.
    pub step_time_ms: f64,
    /// Total cell count in the hypergrid.
    pub total_cells: usize,
    /// Count of alive cells.
    pub alive_count: usize,
    /// 3D projected visual bounding box (size_x, size_y, size_z).
    pub visual_bounds: (usize, usize, usize),
    /// 3D discrete coordinates of all alive cells.
    pub alive_positions: Vec<(usize, usize, usize)>,
}

/// Handle to the background simulation worker.
pub struct SimulationWorker {
    cmd_tx: Sender<SimulationCommand>,
    result_rx: Receiver<SimulationSnapshot>,
    is_computing: Arc<AtomicBool>,
    thread_handle: Option<JoinHandle<()>>,
}

impl SimulationWorker {
    /// Default channel bounds: 32 commands, 2 snapshots.
    pub const DEFAULT_CMD_BOUND: usize = 32;
    pub const DEFAULT_RESULT_BOUND: usize = 2;

    /// Spawns a new simulation worker with default periodic boundary conditions.
    pub fn new(
        epoch: u64,
        size: usize,
        dimensions: usize,
        delta: usize,
        rules: Rules,
        seed: Option<u64>,
        speed_fps: f32,
    ) -> Self {
        Self::new_with_topology(
            epoch,
            size,
            dimensions,
            delta,
            rules,
            true,
            seed,
            speed_fps,
        )
    }

    /// Spawns a new simulation worker with specified periodic boundary topology.
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_topology(
        epoch: u64,
        size: usize,
        dimensions: usize,
        delta: usize,
        rules: Rules,
        periodic: bool,
        seed: Option<u64>,
        speed_fps: f32,
    ) -> Self {
        Self::with_bounds_and_topology(
            epoch,
            size,
            dimensions,
            delta,
            rules,
            periodic,
            seed,
            speed_fps,
            Self::DEFAULT_CMD_BOUND,
            Self::DEFAULT_RESULT_BOUND,
        )
    }

    /// Spawns a worker with custom channel bounds and default periodic boundary conditions.
    #[allow(clippy::too_many_arguments)]
    pub fn with_bounds(
        epoch: u64,
        size: usize,
        dimensions: usize,
        delta: usize,
        rules: Rules,
        seed: Option<u64>,
        speed_fps: f32,
        cmd_bound: usize,
        result_bound: usize,
    ) -> Self {
        Self::with_bounds_and_topology(
            epoch,
            size,
            dimensions,
            delta,
            rules,
            true,
            seed,
            speed_fps,
            cmd_bound,
            result_bound,
        )
    }

    /// Spawns a worker with custom channel bounds and topology.
    #[allow(clippy::too_many_arguments)]
    pub fn with_bounds_and_topology(
        epoch: u64,
        size: usize,
        dimensions: usize,
        delta: usize,
        rules: Rules,
        periodic: bool,
        seed: Option<u64>,
        speed_fps: f32,
        cmd_bound: usize,
        result_bound: usize,
    ) -> Self {
        let (cmd_tx, cmd_rx) = bounded(cmd_bound);
        let (result_tx, result_rx) = bounded(result_bound);
        let is_computing = Arc::new(AtomicBool::new(false));

        let is_computing_clone = is_computing.clone();

        let thread_handle = thread::Builder::new()
            .name("simulation-worker".to_string())
            .spawn(move || {
                worker_loop(
                    epoch,
                    size,
                    dimensions,
                    delta,
                    rules,
                    periodic,
                    seed,
                    speed_fps,
                    is_computing_clone,
                    cmd_rx,
                    result_tx,
                );
            })
            .expect("Failed to spawn simulation worker thread");

        Self {
            cmd_tx,
            result_rx,
            is_computing,
            thread_handle: Some(thread_handle),
        }
    }

    /// Non-blocking receive of the latest completed snapshot.
    pub fn try_recv_snapshot(&self) -> Result<SimulationSnapshot, crossbeam_channel::TryRecvError> {
        self.result_rx.try_recv()
    }

    /// Sends a command to the worker thread without blocking the render loop indefinitely.
    ///
    /// Non-blocking for ordinary events. For critical lifecycle commands (Shutdown,
    /// Reconfigure, Randomize) uses a bounded short timeout (5ms) to guarantee delivery
    /// without ever freezing the UI or render loop.
    pub fn send_command(&self, cmd: SimulationCommand) -> bool {
        match self.cmd_tx.try_send(cmd) {
            Ok(()) => true,
            Err(TrySendError::Full(cmd)) => match cmd {
                SimulationCommand::Shutdown
                | SimulationCommand::Reconfigure { .. }
                | SimulationCommand::Randomize { .. }
                | SimulationCommand::LoadState { .. }
                | SimulationCommand::ExportState { .. } => self
                    .cmd_tx
                    .send_timeout(cmd, Duration::from_millis(50))
                    .is_ok(),
                _ => false, // Drop intermediate rate-limited slider or burst commands
            },
            Err(TrySendError::Disconnected(_)) => false,
        }
    }

    /// Returns whether the worker CPU is currently computing a simulation step.
    pub fn is_computing(&self) -> bool {
        self.is_computing.load(Ordering::Relaxed)
    }

    /// Requests a single simulation step.
    pub fn step(&self) -> bool {
        self.send_command(SimulationCommand::Step)
    }

    /// Toggles or sets automatic playback.
    pub fn set_playing(&self, playing: bool) -> bool {
        self.send_command(SimulationCommand::SetPlaying(playing))
    }

    /// Sets playback speed in generations per second.
    pub fn set_speed(&self, speed_fps: f32) -> bool {
        self.send_command(SimulationCommand::SetSpeed(speed_fps))
    }

    /// Randomizes the grid and resets generation to 0.
    pub fn randomize(&self, epoch: u64, seed: Option<u64>) -> bool {
        self.send_command(SimulationCommand::Randomize { epoch, seed })
    }

    /// Reconfigures grid parameters, dimension, rules and epoch with default periodic boundary conditions.
    #[allow(clippy::too_many_arguments)]
    pub fn reconfigure(
        &self,
        epoch: u64,
        size: usize,
        dimensions: usize,
        delta: usize,
        percent_min: f64,
        percent_max: f64,
        seed: Option<u64>,
    ) -> bool {
        self.reconfigure_with_topology(
            epoch,
            size,
            dimensions,
            delta,
            percent_min,
            percent_max,
            true,
            seed,
        )
    }

    /// Reconfigures grid parameters, dimension, rules, boundary topology and epoch.
    #[allow(clippy::too_many_arguments)]
    pub fn reconfigure_with_topology(
        &self,
        epoch: u64,
        size: usize,
        dimensions: usize,
        delta: usize,
        percent_min: f64,
        percent_max: f64,
        periodic: bool,
        seed: Option<u64>,
    ) -> bool {
        self.send_command(SimulationCommand::Reconfigure {
            epoch,
            size,
            dimensions,
            delta,
            percent_min,
            percent_max,
            periodic,
            seed,
        })
    }

    /// Exports the current simulation state from the worker thread.
    pub fn export_state(&self, timeout: Duration) -> Result<ExportedSimulationState, String> {
        let (reply_tx, reply_rx) = bounded(1);
        if !self.send_command(SimulationCommand::ExportState { reply_tx }) {
            return Err("Worker command channel full or disconnected".to_string());
        }
        match reply_rx.recv_timeout(timeout) {
            Ok(state) => Ok(*state),
            Err(RecvTimeoutError::Timeout) => {
                Err("Timed out waiting for state export from worker thread".to_string())
            }
            Err(RecvTimeoutError::Disconnected) => {
                Err("Worker thread disconnected while waiting for state export".to_string())
            }
        }
    }

    /// Loads a full simulation state into the worker thread.
    pub fn load_state(
        &self,
        epoch: u64,
        delta: usize,
        rules: Rules,
        periodic: bool,
        generation: u64,
        grid: Grid,
    ) -> bool {
        self.send_command(SimulationCommand::LoadState {
            epoch,
            delta,
            rules,
            periodic,
            generation,
            grid,
        })
    }

    /// Shuts down the worker thread and joins it cleanly. Idempotent.
    pub fn shutdown(&mut self) {
        if let Some(handle) = self.thread_handle.take() {
            let _ = self
                .cmd_tx
                .send_timeout(SimulationCommand::Shutdown, Duration::from_millis(100));
            let _ = handle.join();
        }
    }
}

impl Drop for SimulationWorker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Internal mutable state encapsulated on the worker thread.
struct WorkerContext {
    simulation: Simulation,
    delta: usize,
    epoch: u64,
    is_playing: bool,
    speed_fps: f32,
    rng: StdRng,
    next_step_time: Instant,
    pending_snapshot: Option<SimulationSnapshot>,
}

impl WorkerContext {
    /// Builds an exported state snapshot representing current simulation and grid data.
    fn make_exported_state(&self) -> Box<ExportedSimulationState> {
        Box::new(ExportedSimulationState {
            generation: self.simulation.generation,
            dimensions: self.simulation.current.dims.dimensions,
            size: self.simulation.current.dims.size,
            delta: self.delta,
            percent_min: self.simulation.rules.percent_min_neighbors,
            percent_max: self.simulation.rules.percent_max_neighbors,
            periodic: self.simulation.periodic,
            total_cells: self.simulation.current.len(),
            alive_count: self.simulation.current.count_alive(),
            grid: self.simulation.current.clone(),
        })
    }

    /// Builds an immutable snapshot representing current state.
    fn make_snapshot(&self, step_time_ms: f64) -> SimulationSnapshot {
        let total_alive = self.simulation.current.count_alive();
        let alive_positions = collect_alive_positions(&self.simulation.current, self.delta);
        let dims = self.simulation.current.dims;
        let bounds = VisualBounds::compute(dims.dimensions, dims.size, self.delta);
        SimulationSnapshot {
            epoch: self.epoch,
            generation: self.simulation.generation,
            step_time_ms,
            total_cells: self.simulation.current.len(),
            alive_count: total_alive,
            visual_bounds: (bounds.size_x, bounds.size_y, bounds.size_z),
            alive_positions,
        }
    }

    /// Executes one simulation step on CPU and returns the resulting snapshot.
    fn execute_step(&mut self, is_computing: &AtomicBool) -> SimulationSnapshot {
        is_computing.store(true, Ordering::Relaxed);
        let t0 = Instant::now();
        self.simulation.step();
        let step_time_ms = t0.elapsed().as_secs_f64() * 1000.0;
        is_computing.store(false, Ordering::Relaxed);

        self.make_snapshot(step_time_ms)
    }

    /// Attempts to deliver snapshot into result_tx.
    /// If full, stores as pending_snapshot to be delivered when slot opens up.
    fn deliver_or_queue_snapshot(
        &mut self,
        snapshot: SimulationSnapshot,
        result_tx: &Sender<SimulationSnapshot>,
    ) -> bool {
        match result_tx.try_send(snapshot) {
            Ok(()) => true,
            Err(TrySendError::Full(pending)) => {
                self.pending_snapshot = Some(pending);
                true
            }
            Err(TrySendError::Disconnected(_)) => false,
        }
    }
}

/// Background worker loop.
#[allow(clippy::too_many_arguments)]
fn worker_loop(
    initial_epoch: u64,
    initial_size: usize,
    initial_dimensions: usize,
    initial_delta: usize,
    initial_rules: Rules,
    initial_periodic: bool,
    initial_seed: Option<u64>,
    initial_speed_fps: f32,
    is_computing: Arc<AtomicBool>,
    cmd_rx: Receiver<SimulationCommand>,
    result_tx: Sender<SimulationSnapshot>,
) {
    let mut rng = StdRng::seed_from_u64(initial_seed.unwrap_or(42));
    let mut simulation = Simulation::new_with_topology(
        initial_size,
        initial_dimensions,
        initial_rules,
        initial_periodic,
    );
    simulation.randomize(&mut rng);

    let mut ctx = WorkerContext {
        simulation,
        delta: initial_delta,
        epoch: initial_epoch,
        is_playing: false,
        speed_fps: initial_speed_fps.clamp(0.1, 120.0),
        rng,
        next_step_time: Instant::now(),
        pending_snapshot: None,
    };

    // Emit initial generation 0 snapshot
    let initial_snapshot = ctx.make_snapshot(0.0);
    if !ctx.deliver_or_queue_snapshot(initial_snapshot, &result_tx) {
        return;
    }

    loop {
        // CASE 1: A snapshot is pending delivery because channel was full.
        // We MUST deliver it before computing any new steps, guaranteeing strict monotonicity.
        if let Some(pending) = ctx.pending_snapshot.take() {
            crossbeam_channel::select! {
                send(result_tx, pending) -> res => {
                    if res.is_err() {
                        break; // Receiver disconnected
                    }
                    // Delivered successfully! Advance schedule from now
                    let interval = Duration::from_secs_f32(1.0 / ctx.speed_fps.max(0.1));
                    ctx.next_step_time = Instant::now() + interval;
                }
                recv(cmd_rx) -> cmd => {
                    match cmd {
                        Ok(SimulationCommand::Shutdown) => break,
                        Ok(SimulationCommand::Reconfigure {
                            epoch: new_epoch,
                            size,
                            dimensions,
                            delta: new_delta,
                            percent_min,
                            percent_max,
                            periodic,
                            seed,
                        }) => {
                            // Discard obsolete pending snapshot from old configuration!
                            ctx.epoch = new_epoch;
                            ctx.delta = new_delta;
                            let rules = Rules::new(percent_min, percent_max);
                            ctx.simulation.reconfigure_with_topology(size, dimensions, rules, periodic);
                            if let Some(s) = seed {
                                ctx.rng = StdRng::seed_from_u64(s);
                            }
                            let interval = Duration::from_secs_f32(1.0 / ctx.speed_fps.max(0.1));
                            ctx.next_step_time = Instant::now() + interval;
                            let snap0 = ctx.make_snapshot(0.0);
                            if !ctx.deliver_or_queue_snapshot(snap0, &result_tx) {
                                break;
                            }
                        }
                        Ok(SimulationCommand::Randomize { epoch: new_epoch, seed }) => {
                            // Discard obsolete pending snapshot!
                            ctx.epoch = new_epoch;
                            if let Some(s) = seed {
                                ctx.rng = StdRng::seed_from_u64(s);
                            }
                            ctx.simulation.randomize(&mut ctx.rng);
                            ctx.simulation.generation = 0;
                            let interval = Duration::from_secs_f32(1.0 / ctx.speed_fps.max(0.1));
                            ctx.next_step_time = Instant::now() + interval;
                            let snap0 = ctx.make_snapshot(0.0);
                            if !ctx.deliver_or_queue_snapshot(snap0, &result_tx) {
                                break;
                            }
                        }
                        Ok(SimulationCommand::ExportState { reply_tx }) => {
                            let _ = reply_tx.send(ctx.make_exported_state());
                            ctx.pending_snapshot = Some(pending);
                        }
                        Ok(SimulationCommand::LoadState {
                            epoch: new_epoch,
                            delta: new_delta,
                            rules,
                            periodic,
                            generation,
                            grid,
                        }) => {
                            ctx.epoch = new_epoch;
                            ctx.delta = new_delta;
                            ctx.simulation = Simulation::from_grid_with_topology(grid, rules, periodic);
                            ctx.simulation.generation = generation;
                            let interval = Duration::from_secs_f32(1.0 / ctx.speed_fps.max(0.1));
                            ctx.next_step_time = Instant::now() + interval;
                            let snap0 = ctx.make_snapshot(0.0);
                            if !ctx.deliver_or_queue_snapshot(snap0, &result_tx) {
                                break;
                            }
                        }
                        Ok(SimulationCommand::SetPlaying(playing)) => {
                            ctx.is_playing = playing;
                            // Put pending snapshot back to keep waiting for delivery
                            ctx.pending_snapshot = Some(pending);
                        }
                        Ok(SimulationCommand::SetSpeed(new_speed)) => {
                            ctx.speed_fps = new_speed.clamp(0.1, 120.0);
                            ctx.pending_snapshot = Some(pending);
                        }
                        Ok(SimulationCommand::Step) => {
                            // A step is already pending delivery! Keep waiting to deliver it.
                            ctx.pending_snapshot = Some(pending);
                        }
                        Err(_) => break, // Channel disconnected
                    }
                }
            }
            continue;
        }

        // CASE 2: No pending snapshot. Handle idle vs playing states.
        if !ctx.is_playing {
            // IDLE state: sleep waiting for incoming command without consuming CPU
            match cmd_rx.recv() {
                Ok(SimulationCommand::Step) => {
                    let snap = ctx.execute_step(&is_computing);
                    let interval = Duration::from_secs_f32(1.0 / ctx.speed_fps.max(0.1));
                    ctx.next_step_time = Instant::now() + interval;
                    if !ctx.deliver_or_queue_snapshot(snap, &result_tx) {
                        break;
                    }
                }
                Ok(SimulationCommand::SetPlaying(playing)) => {
                    ctx.is_playing = playing;
                    if ctx.is_playing {
                        ctx.next_step_time = Instant::now();
                    }
                }
                Ok(SimulationCommand::SetSpeed(new_speed)) => {
                    ctx.speed_fps = new_speed.clamp(0.1, 120.0);
                }
                Ok(SimulationCommand::Randomize {
                    epoch: new_epoch,
                    seed,
                }) => {
                    ctx.epoch = new_epoch;
                    if let Some(s) = seed {
                        ctx.rng = StdRng::seed_from_u64(s);
                    }
                    ctx.simulation.randomize(&mut ctx.rng);
                    ctx.simulation.generation = 0;
                    let snap0 = ctx.make_snapshot(0.0);
                    if !ctx.deliver_or_queue_snapshot(snap0, &result_tx) {
                        break;
                    }
                }
                Ok(SimulationCommand::Reconfigure {
                    epoch: new_epoch,
                    size,
                    dimensions,
                    delta: new_delta,
                    percent_min,
                    percent_max,
                    periodic,
                    seed,
                }) => {
                    ctx.epoch = new_epoch;
                    ctx.delta = new_delta;
                    let rules = Rules::new(percent_min, percent_max);
                    ctx.simulation.reconfigure_with_topology(size, dimensions, rules, periodic);
                    if let Some(s) = seed {
                        ctx.rng = StdRng::seed_from_u64(s);
                    }
                    let snap0 = ctx.make_snapshot(0.0);
                    if !ctx.deliver_or_queue_snapshot(snap0, &result_tx) {
                        break;
                    }
                }
                Ok(SimulationCommand::ExportState { reply_tx }) => {
                    let _ = reply_tx.send(ctx.make_exported_state());
                }
                Ok(SimulationCommand::LoadState {
                    epoch: new_epoch,
                    delta: new_delta,
                    rules,
                    periodic,
                    generation,
                    grid,
                }) => {
                    ctx.epoch = new_epoch;
                    ctx.delta = new_delta;
                    ctx.simulation = Simulation::from_grid_with_topology(grid, rules, periodic);
                    ctx.simulation.generation = generation;
                    let snap0 = ctx.make_snapshot(0.0);
                    if !ctx.deliver_or_queue_snapshot(snap0, &result_tx) {
                        break;
                    }
                }
                Ok(SimulationCommand::Shutdown) | Err(_) => break,
            }
        } else {
            // PLAYING state: schedule steps according to target speed
            let now = Instant::now();
            let timeout = if now < ctx.next_step_time {
                ctx.next_step_time - now
            } else {
                Duration::ZERO
            };

            if timeout.is_zero() {
                // Perform scheduled generation step
                let snap = ctx.execute_step(&is_computing);
                if !ctx.deliver_or_queue_snapshot(snap, &result_tx) {
                    break;
                }

                // Advance schedule avoiding backlog accumulation
                let interval = Duration::from_secs_f32(1.0 / ctx.speed_fps.max(0.1));
                let finished_at = Instant::now();
                if finished_at > ctx.next_step_time + interval {
                    ctx.next_step_time = finished_at + interval;
                } else {
                    ctx.next_step_time += interval;
                }
            } else {
                // Wait for either next tick or incoming command
                match cmd_rx.recv_timeout(timeout) {
                    Ok(SimulationCommand::Step) => {
                        let snap = ctx.execute_step(&is_computing);
                        let interval = Duration::from_secs_f32(1.0 / ctx.speed_fps.max(0.1));
                        ctx.next_step_time = Instant::now() + interval;
                        if !ctx.deliver_or_queue_snapshot(snap, &result_tx) {
                            break;
                        }
                    }
                    Ok(SimulationCommand::SetPlaying(playing)) => {
                        ctx.is_playing = playing;
                        if ctx.is_playing {
                            ctx.next_step_time = Instant::now();
                        }
                    }
                    Ok(SimulationCommand::SetSpeed(new_speed)) => {
                        ctx.speed_fps = new_speed.clamp(0.1, 120.0);
                    }
                    Ok(SimulationCommand::Randomize {
                        epoch: new_epoch,
                        seed,
                    }) => {
                        ctx.epoch = new_epoch;
                        if let Some(s) = seed {
                            ctx.rng = StdRng::seed_from_u64(s);
                        }
                        ctx.simulation.randomize(&mut ctx.rng);
                        ctx.simulation.generation = 0;
                        let snap0 = ctx.make_snapshot(0.0);
                        if !ctx.deliver_or_queue_snapshot(snap0, &result_tx) {
                            break;
                        }
                    }
                    Ok(SimulationCommand::Reconfigure {
                        epoch: new_epoch,
                        size,
                        dimensions,
                        delta: new_delta,
                        percent_min,
                        percent_max,
                        periodic,
                        seed,
                    }) => {
                        ctx.epoch = new_epoch;
                        ctx.delta = new_delta;
                        let rules = Rules::new(percent_min, percent_max);
                        ctx.simulation.reconfigure_with_topology(size, dimensions, rules, periodic);
                        if let Some(s) = seed {
                            ctx.rng = StdRng::seed_from_u64(s);
                        }
                        let snap0 = ctx.make_snapshot(0.0);
                        if !ctx.deliver_or_queue_snapshot(snap0, &result_tx) {
                            break;
                        }
                    }
                    Ok(SimulationCommand::ExportState { reply_tx }) => {
                        let _ = reply_tx.send(ctx.make_exported_state());
                    }
                    Ok(SimulationCommand::LoadState {
                        epoch: new_epoch,
                        delta: new_delta,
                        rules,
                        periodic,
                        generation,
                        grid,
                    }) => {
                        ctx.epoch = new_epoch;
                        ctx.delta = new_delta;
                        ctx.simulation = Simulation::from_grid_with_topology(grid, rules, periodic);
                        ctx.simulation.generation = generation;
                        let snap0 = ctx.make_snapshot(0.0);
                        if !ctx.deliver_or_queue_snapshot(snap0, &result_tx) {
                            break;
                        }
                    }
                    Ok(SimulationCommand::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
                    Err(RecvTimeoutError::Timeout) => {
                        // Timer expired, ready to compute step in next iteration
                    }
                }
            }
        }
    }
}
