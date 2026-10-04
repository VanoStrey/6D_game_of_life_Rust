//! Transition rules for the 6D Game of Life.
//!
//! Faithfully implements the exact Java math from `LogicGameOfLive.java`:
//! ```java
//! countAllNeighbours = countDieNeighbors + countLiveNeighbors;
//! int minNeighbors = (int) (countAllNeighbours / (100.0 / divisorMinNeighbors));
//! int maxNeighbors = (int) (countAllNeighbours / (100.0 / divisorMaxNeighbors));
//! return !(countLiveNeighbors > maxNeighbors || countLiveNeighbors < minNeighbors);
//! ```

use serde::{Deserialize, Serialize};

/// Cellular automaton transition rules.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rules {
    pub percent_min_neighbors: f64,
    pub percent_max_neighbors: f64,
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            percent_min_neighbors: 20.0,
            percent_max_neighbors: 45.0,
        }
    }
}

impl Rules {
    /// Creates a new `Rules` configuration.
    pub fn new(percent_min_neighbors: f64, percent_max_neighbors: f64) -> Self {
        Self {
            percent_min_neighbors,
            percent_max_neighbors,
        }
    }

    /// Evaluates if a cell should be alive in the next generation based on:
    /// - `count_live`: number of live neighbors found (including self if alive in Java bug mode).
    /// - `count_all`: total number of checked positions within grid bounds.
    #[inline(always)]
    pub fn evaluate(&self, count_live: i32, count_all: i32) -> bool {
        // Java: int minNeighbors = (int) (countAllNeighbours / (100.0 / divisorMinNeighbors));
        let min_neighbors = (count_all as f64 / (100.0 / self.percent_min_neighbors)) as i32;
        // Java: int maxNeighbors = (int) (countAllNeighbours / (100.0 / divisorMaxNeighbors));
        let max_neighbors = (count_all as f64 / (100.0 / self.percent_max_neighbors)) as i32;

        // Java: return !(countLiveNeighbors > maxNeighbors || countLiveNeighbors < minNeighbors);
        !(count_live > max_neighbors || count_live < min_neighbors)
    }

    /// Calculates dynamic min threshold for a given number of total neighbors.
    #[inline(always)]
    pub fn min_neighbors(&self, count_all: i32) -> i32 {
        (count_all as f64 / (100.0 / self.percent_min_neighbors)) as i32
    }

    /// Calculates dynamic max threshold for a given number of total neighbors.
    #[inline(always)]
    pub fn max_neighbors(&self, count_all: i32) -> i32 {
        (count_all as f64 / (100.0 / self.percent_max_neighbors)) as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rules_default_thresholds() {
        let rules = Rules::default();

        // 3D center (count_all = 26)
        assert_eq!(rules.min_neighbors(26), 5); // 26 * 0.20 = 5.2 -> 5
        assert_eq!(rules.max_neighbors(26), 11); // 26 * 0.45 = 11.7 -> 11

        assert!(rules.evaluate(5, 26));
        assert!(rules.evaluate(8, 26));
        assert!(rules.evaluate(11, 26));
        assert!(!rules.evaluate(4, 26));
        assert!(!rules.evaluate(12, 26));
    }

    #[test]
    fn test_rules_size_1_oscillation() {
        let rules = Rules::default();

        // At size = 1, count_all = 1
        assert_eq!(rules.min_neighbors(1), 0); // 1 * 0.20 = 0.2 -> 0
        assert_eq!(rules.max_neighbors(1), 0); // 1 * 0.45 = 0.45 -> 0

        // If dead: count_live = 0 -> in [0, 0] -> lives!
        assert!(rules.evaluate(0, 1));

        // If alive: count_live = 1 -> not in [0, 0] -> dies!
        assert!(!rules.evaluate(1, 1));
    }
}
