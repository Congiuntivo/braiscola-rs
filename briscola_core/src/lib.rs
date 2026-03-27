//! Core Briscola domain model and deterministic game rules.
//!
//! This crate defines shared game primitives used by simulation, AI, and CLI:
//! - Card model and canonical deck ordering.
//! - Compact bitset helpers for seen-card tracking.
//! - Trick resolution and scoring rules.
//! - Public and determinized game-state transitions.

pub mod bitset;
pub mod card;
pub mod rules;
pub mod state;
