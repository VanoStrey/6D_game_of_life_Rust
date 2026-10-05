pub mod coords;
pub mod dump;
pub mod engine;
pub mod grid;
pub mod rules;
pub mod simd_ops;
pub mod simulation;
pub mod worker;

pub use coords::{wrap_coord, Coords6D, GridDimensions};
pub use dump::{DumpError, SimulationDump};
pub use grid::Grid;
pub use rules::Rules;
pub use simulation::{
    compute_cell_state_reference, compute_cell_state_reference_with_topology, step_reference,
    step_reference_with_topology, Simulation,
};
pub use worker::{SimulationCommand, SimulationSnapshot, SimulationWorker};

