//! # simple-chess — library root
//!
//! Declares all modules. The compiler looks for each module in
//! `src/<module_name>.rs`.
//!
//! Making them `pub` lets `main.rs` reference them as
//! `simple_chess::app::App`, etc.

/// Chess rules engine — pure logic, no UI. Wraps shakmaty.
pub mod game;

/// Opening library — loads the lichess TSV dataset and parses PGN.
pub mod openings;

/// Lichess Explorer API client — weighted-random opponent moves.
pub mod explorer;

/// Setup screen — color picker and opening selector.
pub mod setup;

/// Game board — the interactive 8×8 grid component.
pub mod board;

/// Root component — phase management (Setup ↔ Game).
pub mod app;
