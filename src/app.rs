//! Main Application coordinating Simulation Worker, WGPU Renderer, Winit and Egui.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, WindowAttributes, WindowId};

use crate::input::InputState;
use crate::projection::VisualBounds;
use crate::renderer::{create_instances_with_coloring, Camera, GpuState};
use crate::simulation::{
    GridDimensions, Rules, SimulationDump, SimulationSnapshot, SimulationWorker,
};
use crate::ui::{
    max_size_for_dimensions, render_ui, AppSimStatus, DumpFileInfo, UiActions, UiConfig,
    UiDumpState, UiStats,
};

const CUBE_SIZE: f32 = 100.0;

/// Scans the `dumps/` directory for existing `.gol6d` simulation dump files.
fn scan_dumps_dir() -> Vec<DumpFileInfo> {
    let dumps_dir = Path::new("dumps");
    if !dumps_dir.exists() {
        return Vec::new();
    }
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dumps_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "gol6d") {
                if let Ok(meta) = entry.metadata() {
                    let name = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    files.push(DumpFileInfo {
                        path,
                        filename: name,
                        size_bytes: meta.len(),
                    });
                }
            }
        }
    }
    files.sort_by(|a, b| b.filename.cmp(&a.filename));
    files
}

pub struct App<'window> {
    gpu_state: Option<GpuState<'window>>,
    egui_ctx: egui::Context,
    egui_state: Option<egui_winit::State>,
    egui_renderer: Option<egui_wgpu::Renderer>,

    worker: SimulationWorker,
    current_epoch: u64,
    latest_snapshot: Option<SimulationSnapshot>,
    is_reconfiguring: bool,

    camera: Camera,
    input_state: InputState,
    ui_config: UiConfig,
    ui_stats: UiStats,
    dump_state: UiDumpState,
    is_mouse_captured: bool,
    last_speed_fps: f32,
    last_dumps_scan: Instant,

    rng: StdRng,
    last_frame_time: Instant,
    fps_counter: u32,
    fps_timer: Instant,
    current_fps: f32,
}

impl<'window> App<'window> {
    pub fn new() -> Self {
        let ui_config = UiConfig::default();
        let rules = Rules::new(ui_config.percent_min, ui_config.percent_max);
        let epoch = 1;
        let seed = 42;

        let worker = SimulationWorker::new(
            epoch,
            ui_config.size,
            ui_config.dimensions,
            ui_config.delta,
            rules,
            Some(seed),
            ui_config.speed_fps,
        );

        let dims = GridDimensions::new(ui_config.size, ui_config.dimensions);
        let bounds = VisualBounds::compute(dims.dimensions, dims.size, ui_config.delta);
        let total_cells = dims.total_cells;
        let ui_stats = UiStats {
            generation: 0,
            alive_count: 0,
            total_cells,
            instance_count: 0,
            visual_bounds: (bounds.size_x, bounds.size_y, bounds.size_z),
            sim_step_time_ms: 0.0,
            fps: 60.0,
            status: AppSimStatus::Paused,
        };

        let dump_state = UiDumpState {
            last_status: None,
            recent_dumps: scan_dumps_dir(),
        };

        let mut camera = Camera::default();
        camera.frame_bounds((bounds.size_x, bounds.size_y, bounds.size_z), CUBE_SIZE);

        Self {
            gpu_state: None,
            egui_ctx: egui::Context::default(),
            egui_state: None,
            egui_renderer: None,

            worker,
            current_epoch: epoch,
            latest_snapshot: None,
            is_reconfiguring: false,

            camera,
            input_state: InputState::new(),
            last_speed_fps: ui_config.speed_fps,
            last_dumps_scan: Instant::now(),
            ui_config,
            ui_stats,
            dump_state,
            is_mouse_captured: false,

            rng: StdRng::seed_from_u64(seed),
            last_frame_time: Instant::now(),
            fps_counter: 0,
            fps_timer: Instant::now(),
            current_fps: 60.0,
        }
    }

    /// Drains all completed snapshots from the worker channel.
    /// Updates stats and GPU instance buffer with the freshest snapshot matching current_epoch.
    fn drain_worker_snapshots(&mut self) {
        let mut latest_valid: Option<SimulationSnapshot> = None;
        while let Ok(snapshot) = self.worker.try_recv_snapshot() {
            if snapshot.epoch == self.current_epoch {
                latest_valid = Some(snapshot);
            }
        }

        if let Some(snapshot) = latest_valid {
            self.is_reconfiguring = false;
            self.ui_stats.generation = snapshot.generation;
            self.ui_stats.alive_count = snapshot.alive_count;
            self.ui_stats.total_cells = snapshot.total_cells;
            self.ui_stats.instance_count = snapshot.alive_positions.len();
            self.ui_stats.visual_bounds = snapshot.visual_bounds;
            self.ui_stats.sim_step_time_ms = snapshot.step_time_ms;

            let instances = create_instances_with_coloring(
                &snapshot.alive_positions,
                CUBE_SIZE,
                self.ui_config.dimensions,
                self.ui_config.size,
                self.ui_config.delta,
                self.ui_config.color_mode,
            );
            if let Some(gpu) = &mut self.gpu_state {
                gpu.update_instances(&instances);
            }
            self.latest_snapshot = Some(snapshot);
        }
    }

    /// Reconfigures hypergrid parameters and invalidates previous epoch.
    fn reconfigure_simulation(&mut self) {
        self.current_epoch += 1;
        self.is_reconfiguring = true;
        let seed = self.rng.gen();
        self.worker.reconfigure_with_topology(
            self.current_epoch,
            self.ui_config.size,
            self.ui_config.dimensions,
            self.ui_config.delta,
            self.ui_config.percent_min,
            self.ui_config.percent_max,
            self.ui_config.periodic,
            Some(seed),
        );

        let prev_bounds = self.ui_stats.visual_bounds;
        let dims = GridDimensions::new(self.ui_config.size, self.ui_config.dimensions);
        let bounds = VisualBounds::compute(
            dims.dimensions,
            dims.size,
            self.ui_config.delta,
        );
        self.ui_stats.visual_bounds = (bounds.size_x, bounds.size_y, bounds.size_z);

        // Keep the individual cell's visual size on screen invariant across dimension/size adjustments!
        // We shift camera position by the relative change in bounding box center,
        // without resetting camera distance or viewing angles.
        let prev_center = glam::Vec3::new(
            (prev_bounds.0 as f32) * CUBE_SIZE * 0.5,
            (prev_bounds.1 as f32) * CUBE_SIZE * 0.5,
            (prev_bounds.2 as f32) * CUBE_SIZE * 0.5,
        );
        let new_center = glam::Vec3::new(
            (bounds.size_x as f32) * CUBE_SIZE * 0.5,
            (bounds.size_y as f32) * CUBE_SIZE * 0.5,
            (bounds.size_z as f32) * CUBE_SIZE * 0.5,
        );
        let offset = new_center - prev_center;
        self.camera.position += offset;

        let max_dim = ((bounds.size_x.max(bounds.size_y).max(bounds.size_z) as f32) * CUBE_SIZE).max(100.0);
        self.camera.z_far = (max_dim * 12.0).max(100_000.0);
        self.camera.speed_movement = (max_dim * 0.005).clamp(10.0, 1000.0);

        self.ui_stats.total_cells = dims.total_cells;
        self.ui_stats.generation = 0;
    }

    /// Randomizes current grid with a new seed and invalidates previous epoch.
    fn randomize_simulation(&mut self) {
        self.current_epoch += 1;
        self.is_reconfiguring = true;
        let seed = self.rng.gen();
        self.worker.randomize(self.current_epoch, Some(seed));
        self.ui_stats.generation = 0;
    }

    /// Exports the current simulation state to a .gol6d file.
    /// If `target_path` is None, generates a timestamped file in the `dumps/` directory.
    pub fn export_state_to_file(
        &mut self,
        target_path: Option<PathBuf>,
    ) -> Result<PathBuf, String> {
        if self.ui_config.is_playing {
            let msg = "Дамп можно создать только во время паузы!".to_string();
            self.dump_state.last_status = Some((msg.clone(), true));
            return Err(msg);
        }

        let exp = self
            .worker
            .export_state(Duration::from_millis(500))
            .map_err(|e| {
                let msg = format!("Ошибка экспорта: {}", e);
                self.dump_state.last_status = Some((msg.clone(), true));
                msg
            })?;

        let dump = SimulationDump {
            dimensions: exp.dimensions,
            size: exp.size,
            delta: exp.delta,
            percent_min: exp.percent_min,
            percent_max: exp.percent_max,
            periodic: exp.periodic,
            color_mode: self.ui_config.color_mode,
            generation: exp.generation,
            total_cells: exp.total_cells,
            alive_count: exp.alive_count,
            grid: exp.grid,
        };

        let path = match target_path {
            Some(p) => {
                if let Some(parent) = p.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                p
            }
            None => {
                let _ = std::fs::create_dir_all("dumps");
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                PathBuf::from(format!(
                    "dumps/dump_{}d_s{}_gen{}_{}.gol6d",
                    dump.dimensions, dump.size, dump.generation, timestamp
                ))
            }
        };

        let bytes_written = dump.write_to_file(&path).map_err(|e| {
            let msg = format!("Ошибка записи файла: {}", e);
            self.dump_state.last_status = Some((msg.clone(), true));
            msg
        })?;

        self.dump_state.recent_dumps = scan_dumps_dir();
        let file_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let msg = format!(
            "Дамп сохранён: {} ({:.1} КБ, {} живых)",
            file_name,
            bytes_written as f64 / 1024.0,
            dump.alive_count
        );
        self.dump_state.last_status = Some((msg, false));
        Ok(path)
    }

    /// Imports simulation state from a .gol6d file.
    pub fn import_state_from_file(&mut self, path: &Path) -> Result<String, String> {
        let dump = SimulationDump::read_from_file(path).map_err(|e| {
            let msg = format!("Ошибка чтения дампа: {}", e);
            self.dump_state.last_status = Some((msg.clone(), true));
            msg
        })?;

        self.ui_config.dimensions = dump.dimensions;
        self.ui_config.size = dump.size;
        self.ui_config.delta = dump.delta;
        self.ui_config.percent_min = dump.percent_min;
        self.ui_config.percent_max = dump.percent_max;
        self.ui_config.periodic = dump.periodic;
        self.ui_config.color_mode = dump.color_mode;
        self.ui_config.is_playing = false;
        self.worker.set_playing(false);

        self.current_epoch += 1;
        self.is_reconfiguring = true;
        let rules = Rules::new(dump.percent_min, dump.percent_max);
        self.worker.load_state(
            self.current_epoch,
            dump.delta,
            rules,
            dump.periodic,
            dump.generation,
            dump.grid,
        );

        let dims = GridDimensions::new(dump.size, dump.dimensions);
        let bounds = VisualBounds::compute(dump.dimensions, dump.size, dump.delta);
        self.ui_stats.visual_bounds = (bounds.size_x, bounds.size_y, bounds.size_z);
        self.ui_stats.total_cells = dims.total_cells;
        self.ui_stats.generation = dump.generation;
        self.ui_stats.alive_count = dump.alive_count;
        self.ui_stats.instance_count = dump.alive_count;
        self.ui_stats.status = AppSimStatus::Paused;

        self.camera
            .frame_bounds((bounds.size_x, bounds.size_y, bounds.size_z), CUBE_SIZE);

        let file_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let msg = format!(
            "Дамп загружен: {} | {}D (N={}, Δ={}) | Поколение: {} | Живых: {}",
            file_name, dump.dimensions, dump.size, dump.delta, dump.generation, dump.alive_count
        );
        self.dump_state.last_status = Some((msg.clone(), false));
        self.dump_state.recent_dumps = scan_dumps_dir();
        Ok(msg)
    }

    /// Enables or disables video game mouselook mode.
    /// When captured, cursor is hidden and locked, and mouse movement directly rotates camera.
    /// Pressing Escape exits this mode.
    pub fn set_mouse_captured(&mut self, captured: bool) {
        if self.is_mouse_captured == captured {
            return;
        }
        self.is_mouse_captured = captured;
        self.input_state.is_mouse_captured = captured;
        self.input_state.last_cursor_pos = None;

        if let Some(gpu) = &self.gpu_state {
            let window = &gpu.window;
            if captured {
                window.set_cursor_visible(false);
                let _ = window
                    .set_cursor_grab(CursorGrabMode::Locked)
                    .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined));
            } else {
                let _ = window.set_cursor_grab(CursorGrabMode::None);
                window.set_cursor_visible(true);
            }
        }
        self.input_state.clear_frame_deltas();
    }
}

impl<'window> ApplicationHandler for App<'window> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu_state.is_some() {
            return;
        }

        let window_attrs = WindowAttributes::default()
            .with_title("6D Game of Life (Rust / WGPU / Worker / Egui)")
            .with_inner_size(winit::dpi::LogicalSize::new(1920.0, 1080.0));

        let window = Arc::new(
            event_loop
                .create_window(window_attrs)
                .expect("Failed to create window"),
        );

        let gpu_state = pollster::block_on(GpuState::new(window.clone()));

        let egui_state = egui_winit::State::new(
            self.egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );

        let egui_renderer = egui_wgpu::Renderer::new(
            &gpu_state.device,
            gpu_state.config.format,
            egui_wgpu::RendererOptions::default(),
        );

        self.gpu_state = Some(gpu_state);
        self.egui_state = Some(egui_state);
        self.egui_renderer = Some(egui_renderer);

        self.drain_worker_snapshots();
        if let (Some(gpu), Some(snapshot)) = (&mut self.gpu_state, &self.latest_snapshot) {
            if gpu.instance_count == 0 && !snapshot.alive_positions.is_empty() {
                let instances = create_instances_with_coloring(
                    &snapshot.alive_positions,
                    CUBE_SIZE,
                    self.ui_config.dimensions,
                    self.ui_config.size,
                    self.ui_config.delta,
                    self.ui_config.color_mode,
                );
                gpu.update_instances(&instances);
            }
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        self.input_state.process_device_event(&event);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        // 1. If Escape is pressed while in captured camera mode, immediately exit camera mode
        if let WindowEvent::KeyboardInput { ref event, .. } = event {
            if let PhysicalKey::Code(KeyCode::Escape) = event.physical_key {
                if event.state == ElementState::Pressed && self.is_mouse_captured {
                    self.set_mouse_captured(false);
                    return;
                }
            }
        }

        // 2. If window loses focus, immediately release cursor
        if let WindowEvent::Focused(false) = event {
            if self.is_mouse_captured {
                self.set_mouse_captured(false);
            }
        }

        // 3. Process Egui window event (only when mouse is not captured so egui doesn't steal game input)
        let mut egui_consumed = false;
        if !self.is_mouse_captured {
            if let (Some(gpu), Some(egui_state)) = (&self.gpu_state, &mut self.egui_state) {
                let response = egui_state.on_window_event(&gpu.window, &event);
                egui_consumed = response.consumed;
            }
        }

        // 4. Click outside control panel enters video game mouselook mode
        if let WindowEvent::MouseInput {
            state: ElementState::Pressed,
            button,
            ..
        } = event
        {
            if !self.is_mouse_captured && (button == MouseButton::Left || button == MouseButton::Right) {
                let is_over_egui = egui_consumed
                    || self.egui_ctx.is_pointer_over_egui()
                    || self.egui_ctx.egui_wants_pointer_input();
                if !is_over_egui {
                    self.set_mouse_captured(true);
                    return;
                }
            }
        }

        // 5. Only process game input if egui hasn't captured it (or if mouse is captured)
        if !egui_consumed || self.is_mouse_captured {
            self.input_state.process_event(&event);
        }

        match event {
            WindowEvent::CloseRequested => {
                self.worker.shutdown();
                event_loop.exit();
            }
            WindowEvent::Resized(physical_size) => {
                if let Some(gpu) = &mut self.gpu_state {
                    gpu.resize(physical_size);
                    if physical_size.width > 0 && physical_size.height > 0 {
                        self.camera.aspect_ratio =
                            physical_size.width as f32 / physical_size.height as f32;
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let _dt = now.duration_since(self.last_frame_time).as_secs_f32();
                self.last_frame_time = now;

                // FPS accounting
                self.fps_counter += 1;
                if self.fps_timer.elapsed() >= Duration::from_secs(1) {
                    self.current_fps =
                        self.fps_counter as f32 / self.fps_timer.elapsed().as_secs_f32();
                    self.ui_stats.fps = self.current_fps;
                    self.fps_counter = 0;
                    self.fps_timer = Instant::now();
                }

                // 1. Drain snapshots computed by background simulation worker
                self.drain_worker_snapshots();
                let status = if self.is_reconfiguring {
                    AppSimStatus::Reconfiguring
                } else if self.worker.is_computing() {
                    AppSimStatus::Computing
                } else if self.ui_config.is_playing {
                    AppSimStatus::Playing
                } else {
                    AppSimStatus::Paused
                };
                self.ui_stats.status = status;

                // 2. Camera updates from keyboard (Unity Editor style Fly Camera)
                let mut forward = 0.0f32;
                let mut right = 0.0f32;
                let mut up = 0.0f32;

                if self.input_state.is_key_down(KeyCode::KeyW) {
                    forward += 1.0;
                }
                if self.input_state.is_key_down(KeyCode::KeyS) {
                    forward -= 1.0;
                }
                if self.input_state.is_key_down(KeyCode::KeyD) {
                    right += 1.0;
                }
                if self.input_state.is_key_down(KeyCode::KeyA) {
                    right -= 1.0;
                }
                if self.input_state.is_key_down(KeyCode::Space)
                    || self.input_state.is_key_down(KeyCode::KeyE)
                {
                    up += 1.0;
                }
                if self.input_state.is_key_down(KeyCode::ShiftLeft)
                    || self.input_state.is_key_down(KeyCode::ShiftRight)
                    || self.input_state.is_key_down(KeyCode::KeyQ)
                {
                    up -= 1.0;
                }

                if forward != 0.0 || right != 0.0 || up != 0.0 {
                    self.camera.translate_relative(forward, right, up);
                }

                // 3. Camera updates from mouse (video game mouselook)
                let (dx, dy) = self.input_state.effective_mouse_delta();
                if dx != 0.0 || dy != 0.0 {
                    self.camera.rotate_mouse(dx, dy);
                }

                // 4. Hotkeys (logical simulation keys)
                // Enter / N : Step forward 1 generation
                if self.input_state.is_key_just_pressed(KeyCode::Enter)
                    || self.input_state.is_key_just_pressed(KeyCode::NumpadEnter)
                    || self.input_state.is_key_just_pressed(KeyCode::KeyN)
                {
                    self.worker.step();
                }
                // P : Toggle Play / Pause
                if self.input_state.is_key_just_pressed(KeyCode::KeyP) {
                    self.ui_config.is_playing = !self.ui_config.is_playing;
                    self.worker.set_playing(self.ui_config.is_playing);
                }
                // R : Randomize grid
                if self.input_state.is_key_just_pressed(KeyCode::KeyR) {
                    self.randomize_simulation();
                }
                // C / Home : Reset Camera
                if self.input_state.is_key_just_pressed(KeyCode::KeyC)
                    || self.input_state.is_key_just_pressed(KeyCode::Home)
                {
                    let bounds = self.ui_stats.visual_bounds;
                    self.camera.frame_bounds(bounds, CUBE_SIZE);
                }
                // 1..6 : Direct dimension switch
                let dim_keys = [
                    (KeyCode::Digit1, 1),
                    (KeyCode::Digit2, 2),
                    (KeyCode::Digit3, 3),
                    (KeyCode::Digit4, 4),
                    (KeyCode::Digit5, 5),
                    (KeyCode::Digit6, 6),
                    (KeyCode::KeyT, 1), // Legacy shortcut
                ];
                for (key, dim) in dim_keys {
                    if self.input_state.is_key_just_pressed(key) && self.ui_config.dimensions != dim
                    {
                        self.ui_config.dimensions = dim;
                        let max_s = max_size_for_dimensions(dim, self.ui_config.unlock_extreme_size);
                        self.ui_config.size = self.ui_config.size.min(max_s);
                        self.reconfigure_simulation();
                        break;
                    }
                }

                // Escape: exit video game mouselook mode
                if self.input_state.is_key_just_pressed(KeyCode::Escape) && self.is_mouse_captured {
                    self.set_mouse_captured(false);
                }

                self.input_state.clear_frame_deltas();

                // 5. Ensure GPU and Surface Texture are ready BEFORE running Egui UI
                if self.gpu_state.is_none()
                    || self.egui_state.is_none()
                    || self.egui_renderer.is_none()
                {
                    return;
                }

                let (surface_texture, window_clone) = {
                    let gpu = self.gpu_state.as_mut().unwrap();
                    let st = match gpu.surface.get_current_texture() {
                        wgpu::CurrentSurfaceTexture::Success(texture)
                        | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
                        wgpu::CurrentSurfaceTexture::Outdated
                        | wgpu::CurrentSurfaceTexture::Lost => {
                            gpu.resize(gpu.size);
                            return;
                        }
                        wgpu::CurrentSurfaceTexture::Timeout
                        | wgpu::CurrentSurfaceTexture::Occluded => {
                            return;
                        }
                        _ => return,
                    };
                    (st, gpu.window.clone())
                };

                let view = surface_texture
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                // 6. Run Egui UI (guaranteed that GPU frame is ready to render)
                if self.last_dumps_scan.elapsed() > Duration::from_secs(3) {
                    self.dump_state.recent_dumps = scan_dumps_dir();
                    self.last_dumps_scan = Instant::now();
                }

                let raw_input = self
                    .egui_state
                    .as_mut()
                    .unwrap()
                    .take_egui_input(&window_clone);
                let mut config_clone = self.ui_config.clone();
                let stats_clone = self.ui_stats.clone();

                let mut actions = UiActions::default();
                let mut full_output = self.egui_ctx.run_ui(raw_input, |ui| {
                    actions = render_ui(
                        ui.ctx(),
                        &mut config_clone,
                        &stats_clone,
                        &self.dump_state,
                        self.is_mouse_captured,
                    );
                });

                self.ui_config = config_clone;

                if (self.ui_config.speed_fps - self.last_speed_fps).abs() > 0.01 {
                    self.last_speed_fps = self.ui_config.speed_fps;
                    self.worker.set_speed(self.ui_config.speed_fps);
                }

                if actions.toggle_playing {
                    self.worker.set_playing(self.ui_config.is_playing);
                }
                if actions.step {
                    self.worker.step();
                }
                if actions.randomize {
                    self.randomize_simulation();
                }
                if actions.reconfigure {
                    self.reconfigure_simulation();
                }
                if actions.reset_camera {
                    let bounds = self.ui_stats.visual_bounds;
                    self.camera.frame_bounds(bounds, CUBE_SIZE);
                }
                if let Some(new_dim) = actions.switch_dimension {
                    self.ui_config.dimensions = new_dim;
                    let max_s = max_size_for_dimensions(new_dim, self.ui_config.unlock_extreme_size);
                    self.ui_config.size = self.ui_config.size.min(max_s);
                    self.reconfigure_simulation();
                }
                if actions.color_mode_changed {
                    if let (Some(gpu), Some(snapshot)) =
                        (&mut self.gpu_state, &self.latest_snapshot)
                    {
                        let instances = create_instances_with_coloring(
                            &snapshot.alive_positions,
                            CUBE_SIZE,
                            self.ui_config.dimensions,
                            self.ui_config.size,
                            self.ui_config.delta,
                            self.ui_config.color_mode,
                        );
                        gpu.update_instances(&instances);
                    }
                }
                if actions.export_dump_quick {
                    let _ = self.export_state_to_file(None);
                }
                if actions.export_dump_dialog {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("6D Game of Life Dump (*.gol6d)", &["gol6d"])
                        .set_file_name(&format!(
                            "dump_{}d_s{}_gen{}.gol6d",
                            self.ui_config.dimensions, self.ui_config.size, self.ui_stats.generation
                        ))
                        .save_file()
                    {
                        let _ = self.export_state_to_file(Some(path));
                    }
                }
                if actions.import_dump_dialog {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("6D Game of Life Dump (*.gol6d)", &["gol6d"])
                        .pick_file()
                    {
                        let _ = self.import_state_from_file(&path);
                    }
                }
                if let Some(path) = actions.import_dump_path {
                    let _ = self.import_state_from_file(&path);
                }

                // 7. GPU Render Pass
                let gpu = self.gpu_state.as_mut().unwrap();
                let egui_state = self.egui_state.as_mut().unwrap();
                let egui_renderer = self.egui_renderer.as_mut().unwrap();

                gpu.update_camera(&self.camera);

                egui_state.handle_platform_output(&gpu.window, full_output.platform_output);

                let clipped_primitives = self
                    .egui_ctx
                    .tessellate(full_output.shapes, full_output.pixels_per_point);

                let mut encoder =
                    gpu.device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Frame Encoder"),
                        });

                let screen_descriptor = egui_wgpu::ScreenDescriptor {
                    size_in_pixels: [gpu.config.width, gpu.config.height],
                    pixels_per_point: full_output.pixels_per_point,
                };

                for (id, image_deltas) in &full_output.textures_delta.set {
                    // Crucial: texture allocations (pos.is_none()) must happen before partial updates (pos.is_some())
                    let mut sorted_deltas: Vec<_> = image_deltas.iter().collect();
                    sorted_deltas.sort_by_key(|delta| delta.pos.is_some());

                    for image_delta in sorted_deltas {
                        egui_renderer.update_texture(&gpu.device, &gpu.queue, *id, image_delta);
                    }
                }

                egui_renderer.update_buffers(
                    &gpu.device,
                    &gpu.queue,
                    &mut encoder,
                    &clipped_primitives,
                    &screen_descriptor,
                );

                // Pass 1: 3D Instanced Cubes (with depth buffer testing)
                {
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("3D Render Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            depth_slice: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.08,
                                    g: 0.08,
                                    b: 0.12,
                                    a: 1.0,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &gpu.depth_texture_view,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(1.0),
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        occlusion_query_set: None,
                        timestamp_writes: None,
                        multiview_mask: None,
                    });

                    if gpu.instance_count > 0 {
                        render_pass.set_pipeline(&gpu.render_pipeline);
                        render_pass.set_bind_group(0, &gpu.camera_bind_group, &[]);
                        render_pass.set_vertex_buffer(0, gpu.vertex_buffer.slice(..));
                        render_pass.set_vertex_buffer(1, gpu.instance_buffer.slice(..));
                        render_pass.set_index_buffer(
                            gpu.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint16,
                        );
                        render_pass.draw_indexed(0..gpu.num_indices, 0, 0..gpu.instance_count);
                    }
                }

                // Pass 2: Egui 2D Overlay (renders on top of 3D scene, no depth attachment)
                {
                    let mut egui_pass = encoder
                        .begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("Egui UI Render Pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &view,
                                resolve_target: None,
                                depth_slice: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Load,
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: None,
                            occlusion_query_set: None,
                            timestamp_writes: None,
                            multiview_mask: None,
                        })
                        .forget_lifetime();

                    egui_renderer.render(&mut egui_pass, &clipped_primitives, &screen_descriptor);
                }

                for id in &full_output.textures_delta.free {
                    egui_renderer.free_texture(id);
                }
                full_output.textures_delta.clear();

                gpu.queue.submit(std::iter::once(encoder.finish()));
                gpu.queue.present(surface_texture);
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(gpu) = &self.gpu_state {
            gpu.window.request_redraw();
        }
    }
}
