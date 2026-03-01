//! # board.rs — Game Board Component
//!
//! Renders the interactive 8×8 chess board and wires up all game interactivity:
//! - Board orientation (flipped for Black)
//! - Click-to-select / click-to-move with legal move highlights
//! - Computer auto-play through the opening line (with 500 ms delay)
//! - Hint button (shows the human's correct next opening move in violet)
//! - Opening progress display with progressive wrong-move blocking

use leptos::prelude::*;
use leptos::task::spawn_local;
use gloo_timers::future::TimeoutFuture;
use shakmaty::{Color, Role, Square};

use crate::game::{GameState, Outcome};
use crate::openings::{Opening, OpeningMove};


// ── Colour palette ─────────────────────────────────────────────────────────
const LIGHT_SQ:   &str = "#f0d9b5"; // cream — light squares
const DARK_SQ:    &str = "#b58863"; // warm brown — dark squares
const SELECTED:   &str = "#f6f669"; // yellow — selected piece
const CHECK_SQ:   &str = "#e63946"; // red — king in check
const HINT_FROM:  &str = "#7b61ff"; // violet — hint: piece to move
const HINT_TO:    &str = "#a599ff"; // light violet — hint: destination
const BORDER:     &str = "#6b3a2a";
const ACCENT:     &str = "#c9a96e";
const BG:         &str = "#1a0e0a";
const PANEL:      &str = "#2a1a12";
const BTN_BG:     &str = "#6b3a2a";
const TEXT:       &str = "#f0d9b5";

// ── Hint state ─────────────────────────────────────────────────────────────

/// Three-stage hint: Off → show piece to move → show full move.
#[derive(Clone, Copy, Debug, PartialEq)]
enum HintState {
    /// No hint active.
    Off,
    /// From-square highlighted only (which piece to move).
    Piece,
    /// Both from- and to-square highlighted (the full move).
    Full,
}

// ── Piece glyphs ───────────────────────────────────────────────────────────

fn piece_glyph(color: Color, role: Role) -> &'static str {
    match (color, role) {
        (Color::White, Role::King)   => "♔",
        (Color::White, Role::Queen)  => "♕",
        (Color::White, Role::Rook)   => "♖",
        (Color::White, Role::Bishop) => "♗",
        (Color::White, Role::Knight) => "♘",
        (Color::White, Role::Pawn)   => "♙",
        (Color::Black, Role::King)   => "♚",
        (Color::Black, Role::Queen)  => "♛",
        (Color::Black, Role::Rook)   => "♜",
        (Color::Black, Role::Bishop) => "♝",
        (Color::Black, Role::Knight) => "♞",
        (Color::Black, Role::Pawn)   => "♟",
    }
}

fn coords_to_sq(file: u8, rank: u8) -> Square {
    Square::from_coords(
        shakmaty::File::new(file.into()),
        shakmaty::Rank::new(rank.into()),
    )
}

fn sq_file_rank(sq: Square) -> (u8, u8) {
    let idx = sq as u8;
    (idx % 8, idx / 8)
}

// ── ChessBoard component ────────────────────────────────────────────────────

/// Main game board component.
///
/// ## Props
/// - `player_color`: which side the human controls. Determines board orientation
///   and which turns are auto-played.
/// - `opening`: if `Some`, the computer follows this line and hints are available.
///   If `None`, it's free play (no auto-play, no hints).
/// - `on_back`: called when the user clicks "Back to Setup."
#[component]
pub fn ChessBoard(
    /// The human's color. White = standard orientation; Black = flipped board.
    player_color: Color,
    /// The selected opening line, or `None` for free play.
    opening: Option<Opening>,
    /// Opponent rating level (reserved for future Explorer API use).
    #[allow(unused)]
    rating: u16,
    /// Callback fired when user wants to return to the setup screen.
    on_back: impl Fn() + 'static,
    /// Callback fired when user clicks "New Game" — app picks a new variation
    /// and re-mounts the board.
    on_new_game: impl Fn() + 'static,
) -> impl IntoView {

    // ── Signals ─────────────────────────────────────────────────────────────
    let game         = RwSignal::new(GameState::new());
    let selected     = RwSignal::new(Option::<Square>::None);
    let reachable    = RwSignal::new(Vec::<Square>::new());
    let status_msg   = RwSignal::new(initial_status(player_color, &opening, 0));

    // How many opening moves have been applied so far (both sides combined).
    let op_move_idx  = RwSignal::new(0usize);

    // True once the human plays a move that isn't in the opening line.
    let deviated     = RwSignal::new(false);

    // Three-stage hint: Off → Piece (from-sq only) → Full (from+to).
    let hint_state   = RwSignal::new(HintState::Off);

    // Consecutive wrong-move attempts for the current book move.
    // 1st = silent reject, 2nd = highlight piece, 3rd+ = show full move.
    let wrong_attempts = RwSignal::new(0u32);

    // History for undo: snapshots taken BEFORE each human move.
    let history = RwSignal::new(Vec::<(GameState, usize, bool)>::new());

    // Generation counter — incremented on undo/new-game to cancel stale
    // async computer-move tasks (avoids race when user undoes during 500ms delay).
    let generation = RwSignal::new(0u32);

    // Wrap the opening in a StoredValue so closures can access it without
    // fighting the borrow checker. StoredValue is Leptos's way of storing
    // arbitrary non-reactive data inside the reactive system.
    let opening_sv   = StoredValue::new(opening);

    // ── Board orientation ────────────────────────────────────────────────────
    // `player_color` is a prop (plain value, not a signal) so we compute the
    // rank/file orders once and capture them in closures below.
    //
    // White perspective: rank 7 → rank 0 top-to-bottom, file 0 → file 7.
    // Black perspective: rank 0 → rank 7 top-to-bottom, file 7 → file 0.
    let ranks: Vec<u8> = if player_color == Color::White {
        (0u8..8).rev().collect()
    } else {
        (0u8..8).collect()
    };
    let files: Vec<u8> = if player_color == Color::White {
        (0u8..8).collect()
    } else {
        (0u8..8).rev().collect()
    };

    // ── Auto-play Effect ─────────────────────────────────────────────────────
    //
    // Key Leptos concept: `Effect`
    // An `Effect` runs its closure once immediately and again any time a signal
    // it read with `.get()` changes. Think of it as a reactive subscription.
    //
    // Here we watch `game` and `op_move_idx`. Whenever either changes, we check
    // whether it's the computer's turn and schedule an auto-play if so.
    Effect::new(move |_| {
        let state = game.get();           // subscribe: re-run when game changes
        let idx   = op_move_idx.get();    // subscribe: re-run when idx changes
        let dev   = deviated.get();       // subscribe: stop watching after deviation

        // Guard: only auto-play when game is still going.
        if state.outcome() != Outcome::InProgress { return; }
        // Guard: don't auto-play on the human's turn.
        if state.turn() == player_color { return; }
        // Guard: stop auto-play after deviation (free play).
        if dev { return; }

        // Guard: only auto-play if we have an opening with moves remaining.
        let has_moves = opening_sv.with_value(|op| {
            op.as_ref().map(|o| idx < o.moves.len()).unwrap_or(false)
        });
        if !has_moves { return; }

        // Fetch the next book move before entering the async block.
        let next_mv: Option<OpeningMove> = opening_sv.with_value(|op| {
            op.as_ref().and_then(|o| o.moves.get(idx).cloned())
        });
        let Some(book_mv) = next_mv else { return; };

        // Snapshot the generation counter so we can detect if an undo fires
        // during the 500 ms delay and abort this task.
        let gen_snapshot = generation.get_untracked();

        // Clone signal handles — these are cheap (just IDs into Leptos's store).
        let game_h      = game;
        let idx_h       = op_move_idx;
        let status_h    = status_msg;
        let opening_sv2 = opening_sv;
        let generation_h = generation;
        let wrong_h     = wrong_attempts;

        spawn_local(async move {
            TimeoutFuture::new(500).await;

            // Abort if undo or new-game was pressed during the delay.
            if generation_h.get_untracked() != gen_snapshot { return; }

            // Re-verify it's still the computer's turn. The Effect can fire
            // multiple times if several signals change in the same tick
            // (e.g. game + op_move_idx), spawning duplicate tasks with the
            // same gen_snapshot. Only the first task to wake up should act;
            // the rest will see the turn has already flipped.
            let current = game_h.get_untracked();
            if current.turn() == player_color { return; }
            if current.outcome() != Outcome::InProgress { return; }

            // During the opening, always play the book move so the human can
            // practice the full line. No Explorer API call — deterministic
            // and fast.
            game_h.update(|g| { g.apply_move(book_mv.from, book_mv.to); });
            idx_h.update(|i| *i += 1);

            // Update status message after the computer's move.
            let new_idx   = idx_h.get_untracked();
            let total     = opening_sv2.with_value(|op| op.as_ref().map(|o| o.moves.len()).unwrap_or(0));
            let new_state = game_h.get_untracked();
            status_h.set(compute_status(&new_state, player_color, new_idx, total, false));
            // Reset wrong-attempt counter for the human's next move.
            wrong_h.set(0);
        });
    });

    // ── Click handler ────────────────────────────────────────────────────────
    let on_square_click = move |clicked: Square| {
        let current = game.get_untracked();

        // Ignore clicks when it's not the human's turn (computer thinking).
        if current.turn() != player_color { return; }
        // Ignore clicks when the game is over.
        if current.outcome() != Outcome::InProgress { return; }

        // ── Branch: a piece is already selected ──────────────────────────
        if let Some(from) = selected.get_untracked() {
            let is_target = reachable.get_untracked().contains(&clicked);

            if is_target {
                // ── Check book move BEFORE applying ──────────────────
                let idx = op_move_idx.get_untracked();
                let opening_has_moves = opening_sv.with_value(|op| {
                    op.as_ref().map(|o| idx < o.moves.len()).unwrap_or(false)
                });
                let is_opening_move = opening_sv.with_value(|op| {
                    op.as_ref().and_then(|o| o.moves.get(idx)).map(|expected| {
                        from == expected.from && clicked == expected.to
                    }).unwrap_or(false)
                });

                // If we're still in the opening and the move is wrong → REJECT it.
                if opening_has_moves && !is_opening_move && !deviated.get_untracked() {
                    let attempt = wrong_attempts.get_untracked() + 1;
                    wrong_attempts.set(attempt);
                    selected.set(None);
                    reachable.set(vec![]);

                    if attempt == 1 {
                        // 1st wrong: just a status nudge
                        status_msg.set("Not the book move — try again".into());
                    } else if attempt == 2 {
                        // 2nd wrong: highlight the correct piece
                        hint_state.set(HintState::Piece);
                        status_msg.set("Try the highlighted piece".into());
                    } else {
                        // 3rd+: show the full move
                        hint_state.set(HintState::Full);
                        status_msg.set("Play the highlighted move".into());
                    }
                    return;
                }

                // ── Correct move (or past the opening / deviated) ────
                // Save state for undo before applying the move.
                let snap = (
                    game.get_untracked(),
                    op_move_idx.get_untracked(),
                    deviated.get_untracked(),
                );
                history.update(|h| h.push(snap));

                // Apply the human's move.
                game.update(|g| { g.apply_move(from, clicked); });
                selected.set(None);
                reachable.set(vec![]);
                hint_state.set(HintState::Off);
                wrong_attempts.set(0);

                if opening_has_moves && is_opening_move {
                    op_move_idx.update(|i| *i += 1);
                }

                // Update status.
                let new_idx   = op_move_idx.get_untracked();
                let total     = opening_sv.with_value(|op| op.as_ref().map(|o| o.moves.len()).unwrap_or(0));
                let state     = game.get_untracked();
                let dev       = deviated.get_untracked();
                status_msg.set(compute_status(&state, player_color, new_idx, total, dev));
                return;
            }

            // Re-select a different own piece.
            if let Some((c, _)) = current.piece_at(clicked) {
                if c == current.turn() {
                    selected.set(Some(clicked));
                    reachable.set(current.reachable_squares(clicked));
                    return;
                }
            }

            // Deselect.
            selected.set(None);
            reachable.set(vec![]);
            return;
        }

        // ── Branch: nothing selected — try to select ──────────────────────
        if let Some((c, _)) = current.piece_at(clicked) {
            if c == current.turn() {
                selected.set(Some(clicked));
                reachable.set(current.reachable_squares(clicked));
            }
        }
    };

    // ── Square grid renderer ─────────────────────────────────────────────────
    let ranks_sv = StoredValue::new(ranks.clone());
    let files_sv = StoredValue::new(files.clone());

    let squares_view = move || {
        let state   = game.get();
        let sel     = selected.get();
        let reach   = reachable.get();
        let hs      = hint_state.get();
        let idx     = op_move_idx.get();

        // Find the king's square when in check (for red highlight).
        let check_sq: Option<Square> = if state.is_in_check() {
            state.all_pieces().find_map(|(sq, c, r)| {
                (c == state.turn() && r == Role::King).then_some(sq)
            })
        } else {
            None
        };

        // Determine hint squares based on hint stage:
        //   Off   → no highlights
        //   Piece → only from-square (violet)
        //   Full  → from + to squares
        //
        // Guard: only show hint on the human's turn. During the 500 ms
        // computer-thinking delay the turn flips, and `idx` would point at
        // the computer's next move — highlighting the wrong piece.
        let (hint_from_sq, hint_to_sq): (Option<Square>, Option<Square>) = match hs {
            HintState::Off => (None, None),
            HintState::Piece | HintState::Full => {
                if state.turn() != player_color {
                    (None, None)
                } else {
                    opening_sv.with_value(|op| {
                        op.as_ref().and_then(|o| o.moves.get(idx)).map(|mv| {
                            let to_sq = if hs == HintState::Full { Some(mv.to) } else { None };
                            (Some(mv.from), to_sq)
                        }).unwrap_or((None, None))
                    })
                }
            }
        };

        let mut cells = Vec::with_capacity(64);

        ranks_sv.with_value(|rank_order| {
        files_sv.with_value(|file_order| {
        for &display_rank in rank_order {
            for &file in file_order {
                let sq = coords_to_sq(file, display_rank);
                let (_, rank) = sq_file_rank(sq);

                let is_light     = (file + rank) % 2 == 1;
                let is_selected  = sel == Some(sq);
                let is_reachable = reach.contains(&sq);
                let is_check     = check_sq == Some(sq);
                let is_hint_from = hint_from_sq == Some(sq);
                let is_hint_to   = hint_to_sq == Some(sq);

                // Priority order for background colour:
                // selected > check > hint_from/to > normal square colour
                let bg = if is_selected {
                    SELECTED.to_string()
                } else if is_check {
                    CHECK_SQ.to_string()
                } else if is_hint_from {
                    HINT_FROM.to_string()
                } else if is_hint_to {
                    HINT_TO.to_string()
                } else if is_light {
                    LIGHT_SQ.to_string()
                } else {
                    DARK_SQ.to_string()
                };

                // Piece glyph.
                let piece_html = state.piece_at(sq).map(|(color, role)| {
                    let glyph = piece_glyph(color, role);
                    let text_color = if color == Color::White { "#fff" } else { "#1a1a1a" };
                    view! {
                        <span style=format!(
                            "font-size:2.4rem; color:{text_color}; \
                             text-shadow: 0 1px 3px rgba(0,0,0,0.6); \
                             user-select:none; pointer-events:none;"
                        )>
                            {glyph}
                        </span>
                    }
                });

                // Green dot on empty reachable squares.
                let dot_html = (is_reachable && state.piece_at(sq).is_none()).then(|| {
                    view! {
                        <div style="position:absolute; width:33%; height:33%; \
                                    border-radius:50%; background:rgba(0,128,0,0.5); \
                                    pointer-events:none;" />
                    }
                });

                // Green ring on occupied reachable squares (capturable pieces).
                let ring_html = (is_reachable && state.piece_at(sq).is_some()).then(|| {
                    view! {
                        <div style="position:absolute; inset:0; border-radius:50%; \
                                    box-shadow: inset 0 0 0 4px rgba(0,128,0,0.6); \
                                    pointer-events:none;" />
                    }
                });

                cells.push(view! {
                    <div
                        style=format!(
                            "background:{bg}; position:relative; \
                             display:flex; align-items:center; justify-content:center; \
                             cursor:pointer; aspect-ratio:1;"
                        )
                        on:click=move |_| on_square_click(sq)
                    >
                        {dot_html}
                        {ring_html}
                        {piece_html}
                    </div>
                });
            }
        }
        })});

        cells
    };

    // ── Rank / file labels (orientation-aware) ────────────────────────────
    // Labels must mirror the board orientation.
    let rank_labels: Vec<u8> = ranks.iter().map(|&r| r + 1).collect();
    let file_labels: Vec<char> = files.iter().map(|&f| (b'a' + f) as char).collect();

    // ── Opening header (only in opening mode) ─────────────────────────────
    let op_header = opening_sv.with_value(|op| {
        op.as_ref().map(|o| {
            let eco  = o.eco.clone();
            let name = o.name.clone();
            let total = o.moves.len();
            view! {
                <div style=format!(
                    "color:{ACCENT}; font-size:0.8rem; letter-spacing:0.08em; \
                     padding:0.3rem 1rem; border:1px solid {BORDER}; background:{PANEL}; \
                     display:flex; align-items:center; gap:1rem; max-width:100%;"
                )>
                    <span style="font-weight:bold;">{eco}</span>
                    <span style="flex:1;">{name}</span>
                    <span style="opacity:0.7;">
                        {move || {
                            let idx = op_move_idx.get();
                            format!("move {}/{}", idx.min(total), total)
                        }}
                    </span>
                </div>
            }.into_any()
        }).unwrap_or_else(|| view! { <div /> }.into_any())
    });

    // ── Hint button (only in opening mode, only on human's turn) ─────────
    let has_opening = opening_sv.with_value(|op| op.is_some());
    let hint_btn = if has_opening {
        view! {
            <button
                style=move || {
                    let bg = match hint_state.get() {
                        HintState::Off   => BTN_BG,
                        HintState::Piece => "#2a1a6a",   // dark violet — piece selected
                        HintState::Full  => "#4a3a9a",   // bright violet — full move shown
                    };
                    format!(
                        "background:{bg}; color:{TEXT}; border:1px solid {ACCENT}; \
                         padding:0.4rem 1rem; font-family:'Courier New', monospace; \
                         font-size:0.85rem; cursor:pointer; letter-spacing:0.08em;"
                    )
                }
                on:click=move |_| hint_state.update(|h| *h = match *h {
                    HintState::Off   => HintState::Piece,
                    HintState::Piece => HintState::Full,
                    HintState::Full  => HintState::Off,
                })
            >
                {move || match hint_state.get() {
                    HintState::Off   => "💡 Hint",
                    HintState::Piece => "💡 Piece",
                    HintState::Full  => "💡 Move",
                }}
            </button>
        }.into_any()
    } else {
        view! { <span /> }.into_any()
    };

    // ── Full view ─────────────────────────────────────────────────────────
    view! {
        <div style=format!(
            "display:flex; flex-direction:column; align-items:center; \
             font-family:'Courier New', monospace; background:{BG}; \
             min-height:100vh; padding:2rem; gap:0.75rem;"
        )>
            // Title
            <h1 style=format!("color:{ACCENT}; letter-spacing:0.15em; font-size:1.4rem; \
                               text-transform:uppercase; margin:0;")>
                "♟ Simple Chess ♟"
            </h1>

            // Opening ECO + name + move counter (opening mode only)
            {op_header}

            // Status bar
            <div style=format!(
                "color:{TEXT}; font-size:0.9rem; letter-spacing:0.08em; \
                 padding:0.4rem 1.2rem; border:1px solid {BORDER}; background:{PANEL}; \
                 min-width:200px; text-align:center;"
            )>
                {move || status_msg.get()}
            </div>

            // Board frame + labels
            <div style=format!("border:6px solid {BORDER}; box-shadow: 0 0 24px rgba(0,0,0,0.8);")>

                // Rank labels + grid side by side
                <div style="display:grid; grid-template-columns:1.4rem 1fr; gap:0;">

                    // Rank labels (1–8 or 8–1 depending on orientation)
                    <div style="display:grid; grid-template-rows:repeat(8,1fr);">
                        {rank_labels.iter().map(|&r| view! {
                            <div style=format!(
                                "display:flex; align-items:center; justify-content:center; \
                                 color:{ACCENT}; font-size:0.7rem; background:{BG};"
                            )>
                                {r}
                            </div>
                        }).collect_view()}
                    </div>

                    // 8×8 square grid
                    <div style="display:grid; \
                                grid-template-columns:repeat(8,min(10vw,64px)); \
                                grid-template-rows:repeat(8,min(10vw,64px));">
                        {squares_view}
                    </div>
                </div>

                // File labels (a–h or h–a depending on orientation)
                <div style=format!(
                    "display:grid; \
                     grid-template-columns:1.4rem repeat(8,min(10vw,64px)); \
                     background:{BG};"
                )>
                    <div /> // spacer
                    {file_labels.iter().map(|&f| view! {
                        <div style=format!(
                            "display:flex; align-items:center; justify-content:center; \
                             color:{ACCENT}; font-size:0.7rem; height:1.4rem;"
                        )>
                            {f.to_string()}
                        </div>
                    }).collect_view()}
                </div>
            </div>

            // Control buttons row
            <div style="display:flex; gap:0.75rem; flex-wrap:wrap; justify-content:center;">
                {hint_btn}

                <button
                    style=format!(
                        "background:{BTN_BG}; color:{TEXT}; border:1px solid {ACCENT}; \
                         padding:0.4rem 1rem; font-family:'Courier New', monospace; \
                         font-size:0.85rem; cursor:pointer; letter-spacing:0.08em; \
                         text-transform:uppercase;"
                    )
                    on:click=move |_| on_new_game()
                >
                    "New Game"
                </button>

                <button
                    style=move || {
                        let empty = history.get().is_empty();
                        let opacity = if empty { "opacity:0.4; cursor:default;" } else { "" };
                        format!(
                            "background:{BTN_BG}; color:{TEXT}; border:1px solid {ACCENT}; \
                             padding:0.4rem 1rem; font-family:'Courier New', monospace; \
                             font-size:0.85rem; cursor:pointer; letter-spacing:0.08em; \
                             text-transform:uppercase; {opacity}"
                        )
                    }
                    on:click=move |_| {
                        let snap = history.get_untracked().last().cloned();
                        if let Some((g, idx, dev)) = snap {
                            history.update(|h| { h.pop(); });
                            generation.update(|gen| *gen += 1);
                            game.set(g);
                            op_move_idx.set(idx);
                            deviated.set(dev);
                            hint_state.set(HintState::Off);
                            wrong_attempts.set(0);
                            selected.set(None);
                            reachable.set(vec![]);
                            let total = opening_sv.with_value(|op| op.as_ref().map(|o| o.moves.len()).unwrap_or(0));
                            let restored = game.get_untracked();
                            status_msg.set(compute_status(&restored, player_color, idx, total, dev));
                        }
                    }
                >
                    "↩ Undo"
                </button>

                <button
                    style=format!(
                        "background:{PANEL}; color:{ACCENT}; border:1px solid {BORDER}; \
                         padding:0.4rem 1rem; font-family:'Courier New', monospace; \
                         font-size:0.85rem; cursor:pointer; letter-spacing:0.08em; \
                         text-transform:uppercase;"
                    )
                    on:click=move |_| on_back()
                >
                    "← Setup"
                </button>
            </div>
        </div>
    }
}

// ── Helper functions ───────────────────────────────────────────────────────

/// Build the status message shown in the status bar.
///
/// Priority: game-over states > deviation > opening complete > normal turn.
fn compute_status(
    state: &GameState,
    player_color: Color,
    idx: usize,
    total: usize,
    deviated: bool,
) -> String {
    match state.outcome() {
        Outcome::Checkmate { winner } => {
            let who = if winner == Color::White { "White" } else { "Black" };
            return format!("Checkmate! {who} wins!");
        }
        Outcome::Stalemate => return "Stalemate — draw!".into(),
        Outcome::InProgress => {}
    }

    if deviated {
        return "Opponent deviated — free play from here".into();
    }

    if total > 0 && idx >= total {
        return "Opening complete! Free play from here".into();
    }

    let side = if state.turn() == Color::White { "White" } else { "Black" };
    let check = if state.is_in_check() { " — CHECK!" } else { "" };

    if state.turn() == player_color {
        format!("Your turn ({side}){check}")
    } else {
        format!("Computer thinking ({side}){check}")
    }
}

/// Status message shown at the very start of a game.
fn initial_status(player_color: Color, opening: &Option<Opening>, _idx: usize) -> String {
    let side = if player_color == Color::White { "White" } else { "Black" };
    if opening.is_some() {
        if player_color == Color::White {
            format!("Your turn ({side}) — follow the opening!")
        } else {
            // Black means computer (White) moves first
            "Computer thinking (White)".into()
        }
    } else {
        "White to move".into()
    }
}
