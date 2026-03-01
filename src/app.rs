//! # app.rs — Root Component & Phase Manager
//!
//! The `App` component acts as a state machine with two phases:
//!
//! ```text
//! Setup ──[Start Opening]──► Game
//!   ▲                                      │
//!   └────────────[← Setup button]──────────┘
//! ```
//!
//! It owns the phase signal and the "chosen game parameters" signals
//! (player color, selected opening). It renders either `SetupScreen`
//! or `ChessBoard` depending on the current phase.
//!
//! ## Re-roll on New Game
//! When "New Game" is clicked inside ChessBoard, app.rs picks a different
//! random variation from the stored entry pool, updates the opening signal,
//! and increments a `game_gen` counter. Because the Game view branch reads
//! `game_gen.get()`, Leptos re-runs the closure and fully re-mounts
//! ChessBoard with the new opening.

use leptos::prelude::*;
use shakmaty::Color;

use crate::board::ChessBoard;
use crate::openings::{Opening, OpeningEntry, parse_opening};
use crate::setup::{random_index, SetupScreen};

/// The two phases the app can be in.
#[derive(Clone, Debug, PartialEq)]
enum AppPhase {
    Setup,
    Game,
}

/// Root application component.
///
/// Creates and owns the top-level signals, renders phase-appropriate UI.
#[component]
pub fn App() -> impl IntoView {

    // ── Top-level signals ──────────────────────────────────────────────────
    let phase        = RwSignal::new(AppPhase::Setup);
    let player_color = RwSignal::new(Color::White);
    // `Option<Opening>` — None means free play, Some means opening mode.
    let chosen_op    = RwSignal::new(Option::<Opening>::None);
    // Opponent rating for Explorer API move selection.
    let rating       = RwSignal::new(1600u16);
    // Matching entries from the selected pack — used for New Game re-roll.
    let game_entries = RwSignal::new(Vec::<OpeningEntry>::new());
    // Index of the current entry within game_entries — used to avoid repeats.
    let game_entry_idx = RwSignal::new(0usize);
    // Generation counter — incremented on New Game to force ChessBoard re-mount.
    let game_gen     = RwSignal::new(0u32);

    // ── Phase-switching callbacks ──────────────────────────────────────────

    // Called by SetupScreen when user clicks "Start Opening".
    let on_start = move |color: Color, opening: Opening, r: u16, entries: Vec<OpeningEntry>, idx: usize| {
        player_color.set(color);
        chosen_op.set(Some(opening));
        rating.set(r);
        game_entries.set(entries);
        game_entry_idx.set(idx);
        game_gen.set(0);
        phase.set(AppPhase::Game);
    };

    // Called by ChessBoard when user clicks "← Setup".
    let on_back = move || {
        phase.set(AppPhase::Setup);
    };

    // Called by ChessBoard when user clicks "New Game" — picks a different
    // random variation from the same pack and re-mounts the board.
    let on_new_game = move || {
        let entries = game_entries.get_untracked();
        if entries.len() <= 1 { return; }
        let old_idx = game_entry_idx.get_untracked();
        let mut idx = random_index(entries.len());
        while idx == old_idx {
            idx = random_index(entries.len());
        }
        if let Some(opening) = parse_opening(&entries[idx]) {
            game_entry_idx.set(idx);
            chosen_op.set(Some(opening));
            game_gen.update(|g| *g += 1);
        }
    };

    // ── Conditional rendering ──────────────────────────────────────────────
    view! {
        {move || {
            match phase.get() {
                AppPhase::Setup => view! {
                    <SetupScreen
                        on_start=on_start
                    />
                }.into_any(),

                AppPhase::Game => {
                    // Reading game_gen subscribes this closure so it re-runs
                    // (and fully re-mounts ChessBoard) on each New Game click.
                    let _gen = game_gen.get();
                    view! {
                        <ChessBoard
                            player_color=player_color.get()
                            opening=chosen_op.get()
                            rating=rating.get()
                            on_back=on_back
                            on_new_game=on_new_game
                        />
                    }.into_any()
                },
            }
        }}
    }
}
