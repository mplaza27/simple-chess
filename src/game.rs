//! # game.rs — Chess Logic Layer
//!
//! This module is the "brain" of the app. It knows everything about chess rules
//! but nothing about the UI. That separation is intentional: if you ever want to
//! add a CLI, a network mode, or tests, you can reuse this file unchanged.
//!
//! ## Key Rust concept: modules
//! A `mod` is just a namespace. Files named `game.rs` are automatically treated
//! as a module called `game` when you declare `pub mod game;` in `lib.rs`.
//!
//! ## Key Rust concept: `//!` vs `///`
//! `//!` documents the *containing* item (this whole file/module).
//! `///` documents the *next* item (a function, struct, etc.).
//! Both render in `cargo doc`.

// ── Imports (called "use" in Rust) ────────────────────────────────────────
//
// `use` brings names into scope so you don't have to write the full path
// every time. Think of it like Python's `from shakmaty import Chess, Color`.
//
// `shakmaty` is an external crate (Rust's word for "library") that handles
// all the complex chess rules for us: legal move generation, check detection, etc.
use shakmaty::{Chess, Color, EnPassantMode, Move, Position, Role, Square};
use shakmaty::fen::Fen;

// ── Re-exports ─────────────────────────────────────────────────────────────
//
// `pub use` means "import AND re-export". Code in `board.rs` can now write
//   `use crate::game::PieceColor;`
// instead of reaching into shakmaty directly. This gives us a stable internal
// API even if we swap out the chess library someday.
//
// The `as` keyword renames on import, like Python's `import X as Y`.
pub use shakmaty::{Color as PieceColor, Role as PieceRole, Square as ChessSquare};

// ── Outcome ────────────────────────────────────────────────────────────────

/// Describes how a game has ended (or whether it is still going).
///
/// ## Key Rust concept: `enum`
/// Unlike enums in many languages, Rust enums can carry *data*. Each variant
/// is its own shape:
///   - `Checkmate` carries a `winner` field (a `Color` value).
///   - `Stalemate` and `InProgress` carry no data at all.
///
/// This pattern (called a "sum type" or "tagged union") is one of Rust's
/// most powerful features for representing state clearly.
///
/// ## Key Rust concept: `#[derive(...)]`
/// This attribute auto-generates trait implementations for us:
/// - `Clone`  → lets you call `.clone()` to make a copy of the value.
/// - `Debug`  → lets you print it with `{:?}` for debugging.
/// - `PartialEq` → lets you compare two values with `==`.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// The game ended in checkmate. The `winner` field tells us which side won.
    Checkmate { winner: Color },
    /// Neither side has legal moves, but the king is not in check. It's a draw.
    Stalemate,
    /// The game is still being played.
    InProgress,
}

// ── GameState ──────────────────────────────────────────────────────────────

/// The entire state of a chess game, wrapped in a single struct.
///
/// Leptos will store one of these inside an `RwSignal` (a reactive cell).
/// Whenever the signal changes, the UI automatically re-renders.
///
/// ## Key Rust concept: `struct`
/// A struct groups related data together, like a class with only fields
/// (no methods yet — methods go in `impl` blocks below).
///
/// ## Key Rust concept: field visibility
/// `position` has no `pub` keyword, so it is *private*. Only code in this
/// file can read or write it directly. External code must go through our
/// methods. This is called *encapsulation*.
#[derive(Clone, Debug)]
pub struct GameState {
    position: Chess,
}

// ── Methods on GameState ───────────────────────────────────────────────────
//
// Key Rust concept: `impl`
// An `impl` block attaches methods to a type. Think of it like adding
// methods to a class, but written separately from the struct definition.
//
// Methods that take `&self` only *read* the struct (immutable borrow).
// Methods that take `&mut self` can also *modify* it (mutable borrow).
// Methods that take `self` (no `&`) consume/move the value entirely.
impl GameState {

    /// Creates a new game in the standard starting position.
    ///
    /// ## Key Rust concept: `Self`
    /// Inside an `impl` block, `Self` is a shorthand for the type being
    /// implemented — in this case, `GameState`. It makes renaming easier.
    ///
    /// `Chess::default()` returns shakmaty's built-in starting position.
    pub fn new() -> Self {
        Self {
            position: Chess::default(),
        }
    }

    /// Tries to create a `GameState` from a FEN string.
    ///
    /// FEN (Forsyth–Edwards Notation) is a standard text format for describing
    /// a chess position. Example for the start position:
    /// `"rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"`
    ///
    /// ## Key Rust concept: `Option<T>`
    /// `Option<T>` is Rust's way of saying "this might not exist".
    /// - `Some(value)` means "it exists and here it is".
    /// - `None` means "it doesn't exist".
    /// There is no `null` in Rust — you always handle the missing case.
    ///
    /// ## Key Rust concept: the `?` operator
    /// Inside a function returning `Option`, `x?` means:
    /// "If `x` is `None`, return `None` from this function right now.
    ///  If `x` is `Some(v)`, unwrap it and give me `v`."
    /// It's a concise way to propagate "failure" without crashing.
    ///
    /// `.ok()` converts a `Result<T,E>` into `Option<T>`, turning any error
    /// into `None`.
    pub fn from_fen(fen: &str) -> Option<Self> {
        // Try to parse the FEN string. The `?` propagates failure.
        let fen: Fen = fen.parse().ok()?;
        // Try to turn it into a valid chess position.
        let position = fen.into_position(shakmaty::CastlingMode::Standard).ok()?;
        Some(Self { position })
    }

    /// Returns the current position as a FEN string.
    ///
    /// FEN is the standard text format for serializing a chess position.
    /// Used as the cache key for Lichess Explorer API lookups.
    pub fn to_fen(&self) -> String {
        Fen::from_position(&self.position, EnPassantMode::Legal).to_string()
    }

    /// Returns which side (White or Black) is currently to move.
    ///
    /// `&self` means we borrow the `GameState` without taking ownership.
    /// The caller keeps their `GameState` after this call.
    pub fn turn(&self) -> Color {
        self.position.turn()
    }

    /// Returns every legal move available from the current position.
    ///
    /// ## Key Rust concept: `Vec<T>`
    /// A `Vec` (vector) is a growable list — like Python's `list` or
    /// JavaScript's `Array`, but typed. `Vec<Move>` is a list of chess moves.
    ///
    /// `.into_iter()` turns the shakmaty collection into a standard Rust
    /// iterator. `.collect()` gathers iterator items back into a `Vec`.
    pub fn legal_moves(&self) -> Vec<Move> {
        self.position.legal_moves().into_iter().collect()
    }

    /// Returns only the legal moves that start on square `from`.
    ///
    /// ## Key Rust concept: iterator chaining
    /// Rust iterators are lazy — nothing runs until you call `.collect()`.
    /// You can chain adapters like `.filter()` and `.map()` to describe a
    /// pipeline, and they all run together in one pass. Very efficient.
    ///
    /// `.filter(|m| ...)` keeps only items where the closure returns `true`.
    /// `|m|` is a closure (anonymous function) that takes one argument `m`.
    pub fn legal_moves_from(&self, from: Square) -> Vec<Move> {
        self.legal_moves()
            .into_iter()
            // For each move `m`, keep it only if its origin matches `from`.
            // `m.from()` returns `Option<Square>` because some special moves
            // (like dropping a piece) have no origin square.
            .filter(|m| m.from() == Some(from))
            .collect()
    }

    /// Returns the destination squares of all legal moves from `from`.
    ///
    /// The board component calls this to know which squares to highlight
    /// with green dots when the player selects a piece.
    ///
    /// `.map(|m| m.to())` transforms each `Move` into its destination `Square`.
    pub fn reachable_squares(&self, from: Square) -> Vec<Square> {
        self.legal_moves_from(from)
            .into_iter()
            .map(|m| m.to())
            .collect()
    }

    /// Attempts to move a piece from `from` to `to`.
    ///
    /// Returns `true` if the move was legal and applied, `false` otherwise.
    /// Self-checks are automatically rejected because shakmaty only generates
    /// *legal* moves (moves that don't leave your king in check).
    ///
    /// ## Pawn promotion
    /// When a pawn reaches the last rank, chess rules require you to choose
    /// a piece to promote to. We automatically pick Queen (the strongest),
    /// which covers 99% of practical cases.
    ///
    /// ## Key Rust concept: `&mut self`
    /// This method *modifies* the game state (it changes whose turn it is,
    /// updates piece positions, etc.), so it needs `&mut self`.
    pub fn apply_move(&mut self, from: Square, to: Square) -> bool {
        // Collect all legal moves that go from `from` to `to`.
        // There can be more than one only for pawn promotion (you can promote
        // to queen, rook, bishop, or knight — all from the same squares).
        let candidates: Vec<Move> = self
            .legal_moves_from(from)
            .into_iter()
            .filter(|m| m.to() == to)
            .collect();

        // Key Rust concept: `let ... else`
        // This is called a "let-else" pattern. It tries to match the pattern
        // on the left (`Some(chosen)`). If it DOESN'T match (i.e., `candidates`
        // is empty so `.find()` returns `None`), the `else` block runs and we
        // return early with `false`. This avoids deeply nested `if let` chains.
        let Some(chosen) = candidates.into_iter().find(|m| {
            // Prefer queen promotion; accept anything else.
            // `match` in Rust works like `switch` but is exhaustive — the
            // compiler forces you to handle every possible case.
            match m {
                Move::Normal { promotion: Some(r), .. } => *r == Role::Queen,
                // `_` is a wildcard that matches anything not already handled.
                _ => true,
            }
        }) else {
            // No legal move found — the move is illegal.
            return false;
        };

        // We already verified legality, so `play_unchecked` is safe here.
        // It updates `self.position` in place (hence `&mut self` above).
        self.position.play_unchecked(chosen);
        true
    }

    /// Returns the current game outcome.
    ///
    /// After every move the board component calls this to decide what to
    /// show in the status bar.
    pub fn outcome(&self) -> Outcome {
        if self.position.is_checkmate() {
            // The player whose turn it is has no legal moves AND is in check.
            // That means the *other* player just delivered checkmate.
            // `turn()` returns who moves NEXT, so the winner is the opposite.
            let winner = self.position.turn().other();
            Outcome::Checkmate { winner }
        } else if self.position.is_stalemate() {
            Outcome::Stalemate
        } else {
            Outcome::InProgress
        }
    }

    /// Returns `true` if the side to move is currently in check.
    pub fn is_in_check(&self) -> bool {
        self.position.is_check()
    }

    /// Returns the piece on `square`, or `None` if the square is empty.
    ///
    /// The return type `Option<(Color, Role)>` is a tuple inside an Option.
    /// A tuple in Rust is written `(A, B)` — it groups values of potentially
    /// different types, accessed by position: `.0`, `.1`, etc.
    pub fn piece_at(&self, square: Square) -> Option<(Color, Role)> {
        let board = self.position.board();
        // The `?` here: if `piece_at` returns `None` (empty square),
        // we immediately return `None` from our function too.
        let piece = board.piece_at(square)?;
        Some((piece.color, piece.role))
    }

    /// Returns an iterator over every piece on the board as `(square, color, role)`.
    ///
    /// ## Key Rust concept: `impl Trait` return types
    /// `impl Iterator<Item = ...>` means "some type that implements the Iterator
    /// trait" — we don't name the exact type (it's complex), we just promise
    /// it can be iterated. The `+ '_` lifetime annotation ties the iterator's
    /// lifetime to `&self` so it can't outlive the `GameState` it borrows from.
    pub fn all_pieces(&self) -> impl Iterator<Item = (Square, Color, Role)> + '_ {
        self.position
            .board()
            .iter()
            .map(|(sq, piece)| (sq, piece.color, piece.role))
    }
}

// ── Default trait implementation ───────────────────────────────────────────
//
// Key Rust concept: traits
// A `trait` is like an interface — a contract that says "this type can do X".
// `Default` means the type has a sensible zero-argument constructor.
// We delegate to `new()` so both `GameState::default()` and `GameState::new()`
// work identically. Leptos signals require `Default` on the stored type.
impl Default for GameState {
    fn default() -> Self {
        Self::new()
    }
}

// ── Unit tests ─────────────────────────────────────────────────────────────
//
// Key Rust concept: `#[cfg(test)]`
// This attribute tells the compiler to only compile this block when running
// `cargo test`. It never ends up in your final binary, keeping it lean.
//
// Tests live right next to the code they test — no separate test/ directory
// needed (though you can have one). This makes it easy to keep them in sync.
#[cfg(test)]
mod tests {
    // `use super::*` imports everything from the parent module (game.rs).
    // Think of it as "bring everything above into scope for this test module".
    use super::*;
    use shakmaty::Square;

    // Key Rust concept: `#[test]`
    // Any function marked `#[test]` is automatically discovered and run by
    // `cargo test`. If the function panics (via `assert!` failure), the test fails.

    /// A new game should have exactly 32 pieces (16 per side).
    #[test]
    fn new_game_has_32_pieces() {
        let g = GameState::new();
        // `count()` consumes the iterator and counts the items.
        assert_eq!(g.all_pieces().count(), 32);
    }

    /// Chess rules: White always moves first.
    #[test]
    fn white_moves_first() {
        let g = GameState::new();
        assert_eq!(g.turn(), Color::White);
    }

    /// In the starting position, White has exactly 20 legal moves:
    /// 16 pawn moves (each of 8 pawns can go 1 or 2 squares) and
    /// 4 knight moves (each of 2 knights has 2 destinations).
    #[test]
    fn white_has_20_opening_moves() {
        let g = GameState::new();
        assert_eq!(g.legal_moves().len(), 20);
    }

    /// The e2 pawn should be able to move to e3 (one square) and e4 (two squares).
    #[test]
    fn e2_pawn_can_reach_e3_and_e4() {
        let g = GameState::new();
        let e2 = Square::E2;
        let reachable = g.reachable_squares(e2);
        assert!(reachable.contains(&Square::E3));
        assert!(reachable.contains(&Square::E4));
        // Exactly 2 moves — not more, not fewer.
        assert_eq!(reachable.len(), 2);
    }

    /// Moving a pawn three squares is illegal. The game state should be unchanged.
    #[test]
    fn illegal_move_is_rejected() {
        // `mut` marks the variable as mutable. You can't call `&mut self`
        // methods on an immutable variable.
        let mut g = GameState::new();
        let ok = g.apply_move(Square::E2, Square::E5);
        // `assert!` panics (fails the test) if the condition is false.
        assert!(!ok, "e2→e5 should be rejected as illegal");
        // The position should be unchanged — still White's turn.
        assert_eq!(g.turn(), Color::White);
    }

    /// After a legal move, it should be the other side's turn.
    #[test]
    fn legal_move_switches_turn() {
        let mut g = GameState::new();
        let ok = g.apply_move(Square::E2, Square::E4);
        // `assert_eq!` is like `assert!(a == b)` but prints both values on failure.
        assert!(ok);
        assert_eq!(g.turn(), Color::Black);
    }

    /// Scholar's Mate: a famous four-move checkmate sequence.
    /// This verifies the full pipeline: moves apply, turns switch, and
    /// checkmate is correctly detected at the end.
    #[test]
    fn scholars_mate_is_checkmate() {
        let mut g = GameState::new();
        assert!(g.apply_move(Square::E2, Square::E4)); // 1. e4
        assert!(g.apply_move(Square::E7, Square::E5)); // 1... e5
        assert!(g.apply_move(Square::F1, Square::C4)); // 2. Bc4
        assert!(g.apply_move(Square::B8, Square::C6)); // 2... Nc6
        assert!(g.apply_move(Square::D1, Square::H5)); // 3. Qh5
        assert!(g.apply_move(Square::A7, Square::A6)); // 3... a6 (a weak move)
        assert!(g.apply_move(Square::H5, Square::F7)); // 4. Qxf7# checkmate!
        assert_eq!(
            g.outcome(),
            Outcome::Checkmate { winner: Color::White }
        );
    }

    /// A valid FEN string should successfully create a `GameState`.
    #[test]
    fn from_fen_parses_valid_position() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        // `.expect("message")` is like unwrapping an Option but with a
        // helpful error message if it's None — used only in tests/examples.
        let g = GameState::from_fen(fen).expect("valid FEN should parse");
        assert_eq!(g.all_pieces().count(), 32);
    }

    /// Garbage input should gracefully return `None`, not crash.
    #[test]
    fn from_fen_rejects_invalid_string() {
        // `assert!` with `is_none()` verifies we got back None.
        assert!(GameState::from_fen("not a fen").is_none());
    }

    /// Right after a `new()`, the game is still in progress.
    #[test]
    fn outcome_is_in_progress_at_start() {
        let g = GameState::new();
        assert_eq!(g.outcome(), Outcome::InProgress);
    }

    /// `to_fen()` should return the standard starting position FEN.
    #[test]
    fn to_fen_starting_position() {
        let g = GameState::new();
        assert_eq!(
            g.to_fen(),
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
        );
    }

    /// After 1. e4 e5, the FEN should reflect the new position.
    #[test]
    fn to_fen_after_e4_e5() {
        let mut g = GameState::new();
        g.apply_move(Square::E2, Square::E4);
        g.apply_move(Square::E7, Square::E5);
        assert_eq!(
            g.to_fen(),
            "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2"
        );
    }
}
