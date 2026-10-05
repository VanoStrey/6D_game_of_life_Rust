//! 6D Game of Life - High Performance Rust Core and 3D WGPU Renderer
//!
//! A behavioral and mathematical port of the multi-dimensional cellular automaton
//! originally written in Java / JavaFX.

pub mod app;
pub mod input;
pub mod projection;
pub mod renderer;
pub mod simulation;
pub mod ui;

pub use app::App;
pub use projection::{collect_alive_positions, project_coords, VisualBounds};
pub use renderer::{
    create_instances_from_positions, create_instances_with_coloring, Camera, ColorMode, GpuState,
    InstanceRaw,
};
pub use simulation::{
    compute_cell_state_reference, compute_cell_state_reference_with_topology, step_reference,
    step_reference_with_topology, wrap_coord, Coords6D, DumpError, Grid, GridDimensions, Rules,
    Simulation, SimulationCommand, SimulationDump, SimulationSnapshot, SimulationWorker,
};
pub use ui::{render_ui, AppSimStatus, UiActions, UiConfig, UiStats};
