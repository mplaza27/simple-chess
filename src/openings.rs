//! # openings.rs — Chess Opening Library
//!
//! Provides types and functions for loading and parsing the lichess-org
//! chess-openings dataset.
//!
//! ## Two-stage design (for performance)
//!
//! **Stage 1 — cheap async fetch**: `fetch_all_entries()` downloads the five
//! TSV files from the server at runtime and splits them into `OpeningEntry`
//! structs. No chess logic runs. This happens once while the loading screen
//! is shown.
//!
//! **Stage 2 — on demand**: `parse_opening()` converts one entry's PGN string
//! into a `Vec<OpeningMove>` by replaying moves through shakmaty's SAN parser.
//! This runs only when the user clicks "Start Game", so there is no
//! perceived delay even with 3,600+ openings in the dataset.
//!
//! ## Why fetch instead of embed?
//! Using `include_str!` to embed the TSV data added ~370 KB to the WASM
//! binary. Serving the files separately and fetching them with `gloo-net`
//! keeps the WASM binary small and lets the browser cache the TSV files
//! independently.

use shakmaty::{Chess, Position, Role, Square};
use shakmaty::san::San;

// On WASM we use gloo_net for HTTP. The import is gated so the crate is not
// required when compiling natively for `cargo test`.
#[cfg(target_arch = "wasm32")]
use gloo_net::http::Request;

// ── Data types ─────────────────────────────────────────────────────────────

/// A raw opening entry parsed cheaply from a TSV row.
/// Fields are owned `String`s because the data arrives at runtime via HTTP
/// (not embedded at compile time).
#[derive(Clone, Debug, PartialEq)]
pub struct OpeningEntry {
    pub eco: String,   // e.g. "C60"
    pub name: String,  // e.g. "Ruy Lopez"
    pub pgn: String,   // e.g. "1. e4 e5 2. Nf3 Nc6 3. Bb5"
}

/// One move in a parsed opening line.
/// Stores the concrete squares rather than the SAN string, so the board
/// component can apply and compare moves without re-parsing.
#[derive(Clone, Debug, PartialEq)]
pub struct OpeningMove {
    pub from: Square,
    pub to: Square,
    /// Pawn promotion piece, if any. Almost always `None`; very rare in openings.
    pub promotion: Option<Role>,
}

/// A fully parsed opening — has concrete moves the board can apply.
/// Created only when the user clicks "Start Game."
#[derive(Clone, Debug)]
pub struct Opening {
    pub eco: String,
    pub name: String,
    pub pgn: String,
    /// The complete move list for both sides, interleaved:
    /// index 0 = White move 1, index 1 = Black move 1, index 2 = White move 2, …
    pub moves: Vec<OpeningMove>,
}

// ── Public API ─────────────────────────────────────────────────────────────

/// Fetch all five TSV files and return every opening entry.
///
/// On WASM (the browser): downloads the files via HTTP from `/assets/openings/`.
/// The files are served from `dist/assets/openings/` (copied there by Trunk).
///
/// On native (for `cargo test`): reads the same files using `include_str!` so
/// tests can run without a server. The embedded data only ends up in the test
/// binary, not the WASM binary.
pub async fn fetch_all_entries() -> Vec<OpeningEntry> {
    #[cfg(target_arch = "wasm32")]
    {
        let mut out = Vec::with_capacity(3700);
        for file in ["a", "b", "c", "d", "e"] {
            let url = format!("./openings/{file}.tsv");
            if let Ok(resp) = Request::get(&url).send().await {
                if let Ok(text) = resp.text().await {
                    parse_tsv(&text, &mut out);
                }
            }
        }
        out
    }

    // Native fallback: embed data at compile time so `cargo test` works without
    // a running server. These `include_str!` calls only inflate the *test*
    // binary, not the WASM binary (the cfg above excludes this branch there).
    #[cfg(not(target_arch = "wasm32"))]
    {
        const TSV_A: &str = include_str!("../assets/openings/a.tsv");
        const TSV_B: &str = include_str!("../assets/openings/b.tsv");
        const TSV_C: &str = include_str!("../assets/openings/c.tsv");
        const TSV_D: &str = include_str!("../assets/openings/d.tsv");
        const TSV_E: &str = include_str!("../assets/openings/e.tsv");
        let mut out = Vec::with_capacity(3700);
        for tsv in [TSV_A, TSV_B, TSV_C, TSV_D, TSV_E] {
            parse_tsv(tsv, &mut out);
        }
        out
    }
}

/// Convert an `OpeningEntry` into a fully parsed `Opening` with concrete moves.
///
/// Returns `None` if the PGN is malformed or contains moves that shakmaty
/// cannot interpret. In practice this should never fail for the lichess dataset.
pub fn parse_opening(entry: &OpeningEntry) -> Option<Opening> {
    let moves = parse_moves(&entry.pgn)?;
    Some(Opening {
        eco: entry.eco.clone(),
        name: entry.name.clone(),
        pgn: entry.pgn.clone(),
        moves,
    })
}

// ── Private helpers ────────────────────────────────────────────────────────

/// Parse one TSV block (one file's worth of text) and push entries into `out`.
///
/// The TSV format is:
/// ```text
/// eco\tname\tpgn\n
/// A00\tAmar Opening\t1. Nh3\n
/// ```
/// We skip the header row and any blank lines.
fn parse_tsv(tsv: &str, out: &mut Vec<OpeningEntry>) {
    for line in tsv.lines().skip(1) {  // skip the "eco\tname\tpgn" header
        let mut parts = line.splitn(3, '\t');
        if let (Some(eco), Some(name), Some(pgn)) =
            (parts.next(), parts.next(), parts.next())
        {
            out.push(OpeningEntry {
                eco: eco.to_string(),
                name: name.to_string(),
                pgn: pgn.to_string(),
            });
        }
    }
}

/// Parse a PGN move list string into a `Vec<OpeningMove>`.
///
/// PGN looks like: `"1. e4 e5 2. Nf3 Nc6 3. Bb5"`
/// We tokenize it, strip the move-number tokens (`"1."`, `"2."`, etc.),
/// and parse each remaining token as a Standard Algebraic Notation move.
///
/// Returns `None` on any parse or legality error.
fn parse_moves(pgn: &str) -> Option<Vec<OpeningMove>> {
    let mut position = Chess::default();
    let mut moves = Vec::new();

    for token in pgn.split_whitespace() {
        if token.ends_with('.') || token == "*" || token == "1-0"
            || token == "0-1" || token == "1/2-1/2"
        {
            continue;
        }

        let san = San::from_ascii(token.as_bytes()).ok()?;
        let chess_move = san.to_move(&position).ok()?;

        let from = chess_move.from()?;
        let to = chess_move.to();

        let promotion = if let shakmaty::Move::Normal { promotion, .. } = &chess_move {
            *promotion
        } else {
            None
        };

        moves.push(OpeningMove { from, to, promotion });
        position.play_unchecked(chess_move);
    }

    if moves.is_empty() { None } else { Some(moves) }
}

// ── Unit tests ─────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    // Synchronous helper for tests — uses the non-WASM path of fetch_all_entries
    // but drives the async future to completion with a trivial executor.
    fn load_entries() -> Vec<OpeningEntry> {
        // The non-WASM branch of fetch_all_entries is synchronous (include_str!),
        // so we can poll the future once and it will be immediately ready.
        use std::future::Future;
        use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
        fn noop(_: *const ()) {}
        fn noop_clone(_: *const ()) -> RawWaker { RawWaker::new(std::ptr::null(), &VTABLE) }
        static VTABLE: RawWakerVTable = RawWakerVTable::new(noop_clone, noop, noop, noop);
        let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
        let mut cx = Context::from_waker(&waker);
        let mut fut = Box::pin(fetch_all_entries());
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => v,
            Poll::Pending => panic!("fetch_all_entries should be immediately ready on native"),
        }
    }

    #[test]
    fn loads_nonzero_entries() {
        let entries = load_entries();
        assert!(entries.len() > 3000, "expected 3000+ entries, got {}", entries.len());
    }

    #[test]
    fn first_entry_is_amar_opening() {
        let entries = load_entries();
        let first = &entries[0];
        assert_eq!(first.eco, "A00");
        assert_eq!(first.name, "Amar Opening");
    }

    #[test]
    fn parse_single_move_opening() {
        let entry = OpeningEntry {
            eco: "A00".to_string(),
            name: "Amar Opening".to_string(),
            pgn: "1. Nh3".to_string(),
        };
        let opening = parse_opening(&entry).expect("should parse");
        assert_eq!(opening.moves.len(), 1);
        assert_eq!(opening.moves[0].from, Square::G1);
        assert_eq!(opening.moves[0].to, Square::H3);
    }

    #[test]
    fn parse_ruy_lopez() {
        let entry = OpeningEntry {
            eco: "C60".to_string(),
            name: "Ruy Lopez".to_string(),
            pgn: "1. e4 e5 2. Nf3 Nc6 3. Bb5".to_string(),
        };
        let opening = parse_opening(&entry).expect("should parse");
        assert_eq!(opening.moves.len(), 5);
        assert_eq!(opening.moves[0].from, Square::E2);
        assert_eq!(opening.moves[0].to, Square::E4);
        assert_eq!(opening.moves[4].from, Square::F1);
        assert_eq!(opening.moves[4].to, Square::B5);
    }

    #[test]
    fn invalid_pgn_returns_none() {
        let entry = OpeningEntry {
            eco: "X00".to_string(),
            name: "Bad".to_string(),
            pgn: "1. Zz99".to_string(),
        };
        assert!(parse_opening(&entry).is_none());
    }

    #[test]
    fn all_entries_have_nonempty_fields() {
        for entry in load_entries() {
            assert!(!entry.eco.is_empty());
            assert!(!entry.name.is_empty());
            assert!(!entry.pgn.is_empty());
        }
    }

    // ── Queen's Gambit ─────────────────────────────────────────────────────

    fn is_queens_gambit(e: &OpeningEntry) -> bool {
        // Use starts_with to exclude false matches like
        // "Zukertort Opening: Queen's Gambit Invitation".
        e.name.starts_with("Queen's Gambit")
            || e.name.starts_with("Slav Defense")
            || e.name.starts_with("Semi-Slav Defense")
    }

    #[test]
    fn qg_entries_exist_in_dataset() {
        let entries = load_entries();
        let qg: Vec<_> = entries.iter().filter(|e| is_queens_gambit(e)).collect();
        assert!(qg.len() > 100, "expected 100+ QG entries, got {}", qg.len());
    }

    #[test]
    fn qg_all_entries_parse() {
        // Every QG entry in the dataset should produce a valid move list.
        let entries = load_entries();
        let qg_entries: Vec<_> = entries.iter().filter(|e| is_queens_gambit(e)).collect();
        let failed: Vec<_> = qg_entries.iter()
            .filter(|e| parse_opening(e).is_none())
            .map(|e| e.name.as_str())
            .collect();
        assert!(failed.is_empty(), "failed to parse {} QG entries: {:?}", failed.len(), failed);
    }

    #[test]
    fn qg_canonical_move_sequence() {
        // Verify the canonical QG move order using known PGNs.
        // (Dataset entries may reach positions via transposition, so we
        //  test move structure with hand-written PGNs rather than sweeping
        //  the full dataset.)
        let cases = [
            ("Queen's Gambit Declined", "D30", "1. d4 d5 2. c4 e6"),
            ("Queen's Gambit Accepted", "D20", "1. d4 d5 2. c4 dxc4"),
            ("Slav Defense",            "D10", "1. d4 d5 2. c4 c6"),
            ("Semi-Slav Defense",       "D43", "1. d4 d5 2. c4 c6 3. Nf3 Nf6 4. Nc3 e6"),
        ];
        for (name, eco, pgn) in cases {
            let entry = OpeningEntry { eco: eco.to_string(), name: name.to_string(), pgn: pgn.to_string() };
            let opening = parse_opening(&entry).expect(name);
            // All start 1. d4
            assert_eq!(opening.moves[0].from, Square::D2, "{name}  move 0 from");
            assert_eq!(opening.moves[0].to,   Square::D4, "{name}  move 0 to");
            // 1... d5
            assert_eq!(opening.moves[1].from, Square::D7, "{name}  move 1 from");
            assert_eq!(opening.moves[1].to,   Square::D5, "{name}  move 1 to");
            // 2. c4
            assert_eq!(opening.moves[2].from, Square::C2, "{name}  move 2 from");
            assert_eq!(opening.moves[2].to,   Square::C4, "{name}  move 2 to");
        }
    }

    #[test]
    fn qg_declined_has_e6_or_c6_response() {
        // QGD: Black declines with 2...e6 or 2...c6 (Slav).
        let entries = load_entries();
        let qgd: Vec<_> = entries.iter()
            .filter(|e| e.name.contains("Queen's Gambit Declined") && !e.name.contains(':'))
            .collect();
        assert!(!qgd.is_empty(), "no bare QGD entries found");
        for entry in qgd {
            let opening = parse_opening(entry).unwrap();
            // move 3 = Black's 2nd move
            let black_response = &opening.moves[3];
            let to = black_response.to;
            assert!(
                to == Square::E6 || to == Square::C6,
                "'{}'  Black's 2nd move should be e6 or c6, got {:?}", entry.name, to
            );
        }
    }

    #[test]
    fn qg_accepted_black_captures_c4() {
        // QGA: Black's second move (2...dxc4) must land on c4.
        // Use a hand-known PGN so the test is independent of dataset quirks.
        let entry = OpeningEntry {
            eco: "D20".to_string(),
            name: "Queen's Gambit Accepted".to_string(),
            pgn: "1. d4 d5 2. c4 dxc4".to_string(),
        };
        let opening = parse_opening(&entry).unwrap();
        assert_eq!(opening.moves.len(), 4, "QGA should have 4 half-moves");
        // move index 3 = Black's 2nd move: dxc4
        assert_eq!(opening.moves[3].from, Square::D5, "dxc4 should start on d5");
        assert_eq!(opening.moves[3].to,   Square::C4, "dxc4 should land on c4");
    }

    #[test]
    fn slav_defense_entries_exist() {
        let entries = load_entries();
        let slav: Vec<_> = entries.iter()
            .filter(|e| e.name.starts_with("Slav Defense"))
            .collect();
        assert!(slav.len() > 10, "expected 10+ Slav Defense entries, got {}", slav.len());
    }

    // ── Modern Defense ─────────────────────────────────────────────────────

    fn is_modern_defense(e: &OpeningEntry) -> bool {
        // Require the name to START with "Modern Defense" to exclude false
        // matches like "Hungarian Opening: Reversed Modern Defense".
        e.name.starts_with("Modern Defense")
    }

    #[test]
    fn modern_defense_entries_exist_in_dataset() {
        let entries = load_entries();
        let md: Vec<_> = entries.iter().filter(|e| is_modern_defense(e)).collect();
        assert!(md.len() > 20, "expected 20+ Modern Defense entries, got {}", md.len());
    }

    #[test]
    fn modern_defense_all_entries_parse() {
        let entries = load_entries();
        let md_entries: Vec<_> = entries.iter().filter(|e| is_modern_defense(e)).collect();
        let failed: Vec<_> = md_entries.iter()
            .filter(|e| parse_opening(e).is_none())
            .map(|e| e.name.as_str())
            .collect();
        assert!(failed.is_empty(), "failed to parse {} Modern Defense entries: {:?}", failed.len(), failed);
    }

    #[test]
    fn modern_defense_black_plays_g6_first() {
        // Verify g6 is Black's first move in canonical Modern Defense PGNs
        // across all White first-move options. The dataset contains some
        // transposition variants (e.g. Semi-Averbakh, Polish Variation) where
        // the canonical move order differs, so we test with known PGNs here.
        let cases = [
            ("Modern Defense vs 1.e4",  "B06", "1. e4 g6 2. d4 Bg7"),
            ("Modern Defense vs 1.d4",  "A41", "1. d4 g6 2. c4 Bg7"),
            ("Modern Defense vs 1.Nf3", "A04", "1. Nf3 g6 2. d4 Bg7"),
        ];
        for (name, eco, pgn) in cases {
            let entry = OpeningEntry { eco: eco.to_string(), name: name.to_string(), pgn: pgn.to_string() };
            let opening = parse_opening(&entry).expect(name);
            // moves[1] = Black's first half-move = g6
            let black_first = &opening.moves[1];
            assert_eq!(black_first.from, Square::G7, "{name}  Black g6 should start on g7");
            assert_eq!(black_first.to,   Square::G6, "{name}  Black's first move should be g6");
            // moves[3] = Black's second half-move = Bg7
            if opening.moves.len() >= 4 {
                let black_second = &opening.moves[3];
                assert_eq!(black_second.from, Square::F8, "{name}  Bg7 should start on f8");
                assert_eq!(black_second.to,   Square::G7, "{name}  Bg7 should land on g7");
            }
        }
    }

    #[test]
    fn modern_defense_covers_multiple_white_first_moves() {
        // The dataset should include Modern Defense lines against at least
        // 1.e4, 1.d4, and 1.Nf3 (all start with different White first moves).
        let entries = load_entries();
        let md_entries: Vec<_> = entries.iter().filter(|e| is_modern_defense(e)).collect();
        let white_first_moves: std::collections::HashSet<Square> = md_entries.iter()
            .filter_map(|e| parse_opening(e))
            .filter(|o| !o.moves.is_empty())
            .map(|o| o.moves[0].to)
            .collect();
        // Should see pawns landing on e4, d4, and at least one more square.
        assert!(white_first_moves.contains(&Square::E4),
            "Modern Defense should include vs 1.e4");
        assert!(white_first_moves.contains(&Square::D4),
            "Modern Defense should include vs 1.d4");
        assert!(white_first_moves.len() >= 3,
            "Modern Defense should cover ≥3 different White first moves, got {:?}",
            white_first_moves);
    }

    #[test]
    fn modern_defense_bishop_fianchetto_in_longer_lines() {
        // Lines with 4+ half-moves should include Bg7 (f8→g7).
        let entries = load_entries();
        let md_entries: Vec<_> = entries.iter().filter(|e| is_modern_defense(e)).collect();
        let has_bg7 = md_entries.iter()
            .filter_map(|e| parse_opening(e))
            .filter(|o| o.moves.len() >= 4)
            .any(|o| o.moves[3].from == Square::F8 && o.moves[3].to == Square::G7);
        assert!(has_bg7, "expected at least one Modern Defense line with Bg7 (f8→g7) on move 2");
    }

    // ── Italian Game ──────────────────────────────────────────────────────

    fn is_italian_game(e: &OpeningEntry) -> bool {
        e.name.starts_with("Italian Game")
    }

    #[test]
    fn italian_game_entries_exist_in_dataset() {
        let entries = load_entries();
        let ig: Vec<_> = entries.iter().filter(|e| is_italian_game(e)).collect();
        assert!(ig.len() > 100, "expected 100+ Italian Game entries, got {}", ig.len());
    }

    #[test]
    fn italian_game_all_entries_parse() {
        let entries = load_entries();
        let ig_entries: Vec<_> = entries.iter().filter(|e| is_italian_game(e)).collect();
        let failed: Vec<_> = ig_entries.iter()
            .filter(|e| parse_opening(e).is_none())
            .map(|e| e.name.as_str())
            .collect();
        assert!(failed.is_empty(), "failed to parse {} Italian Game entries: {:?}", failed.len(), failed);
    }

    #[test]
    fn italian_game_canonical_move_sequence() {
        // Verify the canonical Italian Game move order using known PGNs.
        // (Some dataset entries like "Scotch Gambit, Canal Variation" reach
        //  the position via transposition, so we test with hand-written PGNs.)
        let cases = [
            ("Italian Game",                   "C50", "1. e4 e5 2. Nf3 Nc6 3. Bc4"),
            ("Italian Game: Giuoco Piano",     "C53", "1. e4 e5 2. Nf3 Nc6 3. Bc4 Bc5"),
            ("Italian Game: Evans Gambit",      "C51", "1. e4 e5 2. Nf3 Nc6 3. Bc4 Bc5 4. b4"),
            ("Italian Game: Two Knights Defense","C55", "1. e4 e5 2. Nf3 Nc6 3. Bc4 Nf6"),
        ];
        for (name, eco, pgn) in cases {
            let entry = OpeningEntry { eco: eco.to_string(), name: name.to_string(), pgn: pgn.to_string() };
            let opening = parse_opening(&entry).expect(name);
            // All start 1. e4
            assert_eq!(opening.moves[0].from, Square::E2, "{name} move 0 from");
            assert_eq!(opening.moves[0].to,   Square::E4, "{name} move 0 to");
            // 1... e5
            assert_eq!(opening.moves[1].from, Square::E7, "{name} move 1 from");
            assert_eq!(opening.moves[1].to,   Square::E5, "{name} move 1 to");
            // 2. Nf3
            assert_eq!(opening.moves[2].from, Square::G1, "{name} move 2 from");
            assert_eq!(opening.moves[2].to,   Square::F3, "{name} move 2 to");
            // 2... Nc6
            assert_eq!(opening.moves[3].from, Square::B8, "{name} move 3 from");
            assert_eq!(opening.moves[3].to,   Square::C6, "{name} move 3 to");
            // 3. Bc4
            assert_eq!(opening.moves[4].from, Square::F1, "{name} move 4 from");
            assert_eq!(opening.moves[4].to,   Square::C4, "{name} move 4 to");
        }
    }

    // ── Caro-Kann Defense ─────────────────────────────────────────────────

    fn is_caro_kann(e: &OpeningEntry) -> bool {
        e.name.starts_with("Caro-Kann Defense")
    }

    #[test]
    fn caro_kann_entries_exist_in_dataset() {
        let entries = load_entries();
        let ck: Vec<_> = entries.iter().filter(|e| is_caro_kann(e)).collect();
        assert!(ck.len() > 80, "expected 80+ Caro-Kann entries, got {}", ck.len());
    }

    #[test]
    fn caro_kann_all_entries_parse() {
        let entries = load_entries();
        let ck_entries: Vec<_> = entries.iter().filter(|e| is_caro_kann(e)).collect();
        let failed: Vec<_> = ck_entries.iter()
            .filter(|e| parse_opening(e).is_none())
            .map(|e| e.name.as_str())
            .collect();
        assert!(failed.is_empty(), "failed to parse {} Caro-Kann entries: {:?}", failed.len(), failed);
    }

    #[test]
    fn caro_kann_canonical_move_sequence() {
        // All Caro-Kann lines start 1.e4 c6
        let entries = load_entries();
        let ck_entries: Vec<_> = entries.iter().filter(|e| is_caro_kann(e)).collect();
        assert!(!ck_entries.is_empty(), "no Caro-Kann entries found");
        for entry in ck_entries {
            let opening = parse_opening(entry).unwrap();
            assert!(opening.moves.len() >= 2,
                "'{}' has fewer than 2 half-moves", entry.name);
            // 1. e4
            assert_eq!(opening.moves[0].from, Square::E2, "'{}' move 0 from", entry.name);
            assert_eq!(opening.moves[0].to,   Square::E4, "'{}' move 0 to", entry.name);
            // 1... c6
            assert_eq!(opening.moves[1].from, Square::C7, "'{}' move 1 from", entry.name);
            assert_eq!(opening.moves[1].to,   Square::C6, "'{}' move 1 to", entry.name);
        }
    }
}
