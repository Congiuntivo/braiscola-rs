//! Briscola AI based on Monte Carlo simulation over hidden information.
//!
//! This crate evaluates legal moves from a public game view by repeatedly:
//! 1) sampling a consistent hidden world,
//! 2) forcing a candidate move,
//! 3) rolling out to terminal state with a lightweight policy, and
//! 4) aggregating win probability and score delta.
//!
//! Modules:
//! - [determinize]: builds sampled hidden states from public information.
//! - [mc]: move selection entry point and simulation aggregation.
//! - [rollout]: deterministic policy used during playouts.
//! - [rng]: fast RNG utilities used for sampling and shuffling.

pub mod determinize;
pub mod mc;
pub mod rng;
pub mod rollout;
