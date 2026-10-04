pub mod coords;
pub mod engine;
pub mod grid;
pub mod rules;
pub mod simd_ops;
pub mod simulation;
pub mod worker;

pub use coords::{Coords6D, GridDimensions};
pub use grid::Grid;
pub use rules::Rules;
pub use simulation::Simulation;
pub use worker::{SimulationCommand, SimulationSnapshot, SimulationWorker};
