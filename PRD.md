# Simple Chess — Product Requirements Document

## 1. Goal
Build a high-performance web-based chess app using Leptos 0.7 (Frontend) and
Shakmaty (Logic). The project is organized to separate "Game Rules" from "UI
Rendering."

The primary use-case is **adaptive opening training**: the user picks a side,
a named opening, and a difficulty level (800–2500), then plays through it move
by move. The computer uses real game data from the **Lichess Opening Explorer
API** to simulate realistic opponents — playing common alternatives at
frequencies matching actual games at the selected rating band. Below 1600,
extra randomness makes the opponent less predictable. The result is a training
tool that feels like practicing against a real player, not memorizing a fixed
line.

---

## 2. Tech Stack

| Concern       | Tool / Crate                                      |
|---------------|---------------------------------------------------|
| Language      | Rust (latest stable)                              |
| Frontend      | Leptos 0.7 — CSR (pure WASM, no server)           |
| Chess Logic   | shakmaty v0.30+ (move gen, check, SAN parsing)    |
| Opening Data  | lichess-org/chess-openings TSV dataset (runtime-fetched) |
| Async timers  | gloo-timers v0.3 (computer move delay)            |
| Build tool    | Trunk (WASM bundler)                              |
| Styling       | Inline CSS — retro warm brown palette             |
| Move frequency data | Lichess Opening Explorer API (runtime, free, no auth) |
| JSON parsing  | serde + serde_json (WASM-only, for API responses + cache) |
| Browser storage | gloo-storage (localStorage for explorer cache + bookmarks) |
| HTTP fetch    | gloo-net v0.6 (already present — reuse for Explorer API) |

---

## 3. Core Logic Requirements

- Maintain a reactive chess position using `RwSignal<GameState>`.
- Validate all moves via `shakmaty::Position` (self-checks auto-rejected).
- Support: piece movement, captures, castling, en passant, pawn promotion (auto-queen).
- Detect Checkmate and Stalemate reactively after every move.
- Parse chess openings from embedded lichess TSV data (eco, name, PGN columns).
- Convert PGN algebraic notation into concrete `(Square, Square)` move pairs using
  `shakmaty::san::San::to_move()`.
- Expose `GameState::to_fen()` for Explorer API queries (delegates to shakmaty's FEN support).
- Weighted random move selection given Explorer frequency data.

---

## 4. Application Phases

### Phase A — Setup Screen
Shown on first load and after "Back to Setup" is pressed.

1. **Side selector**: White | Black toggle buttons.
   - Determines which side the human controls and sets board orientation.
2. **Available Opening Packs** (★ Featured): four curated packs — Queen's Gambit, Modern Defense, Italian Game, and Caro-Kann Defense.
   - Cards pinned at top. Click to select; "▶ Start Opening" becomes enabled.
   - Each pack randomly picks a variation from the matching dataset subset on game start.
3. **Action buttons**:
   - **▶ Start Opening** — begins a game with the selected pack/opening and side.
   - **Free Play** — begins a standard free game (no opening guidance, always enabled).
4. **Rating slider** (800–2500): configures opponent difficulty. Default 1600.
5. **Request an Opening** section (replaces the full dataset browser):
   - Search box: hidden until user types ≥ 1 character.
   - Autocomplete against a curated list of 10 well-known opening groups.
   - Available groups (QG, MD, Italian Game, Caro-Kann) show "★ Available" — clicking one selects the featured pack.
   - Unavailable groups show "Not yet available" — clicking shows a request panel.
   - "📩 Request something else" row → free-text request panel.
   - Rate-limited: max 2 requests per user per week (localStorage timestamps).
   - Global queue: developer manually closes requests by setting `REQUESTS_OPEN = false`.
   - Submissions POSTed to Formspree endpoint → email notification to developer.
   - No requester email required.

### Phase B — Game Screen
Shown after the user starts a game.

1. **Board**: 8×8 CSS grid with Unicode chess pieces.
   - White's perspective: rank 8 at top, file `a` on left.
   - Black's perspective: rank 1 at top, file `h` on left (board flipped).
2. **Opening header** (opening mode only): ECO code + opening name + move counter
   e.g. `"C60 Ruy Lopez — Move 2 of 5"`.
3. **Status bar**: current player, check warnings, game-over messages,
   deviation notice.
4. **Interactivity**: click-to-select → green-dot highlights → click-to-move.
5. **Hint button** (opening mode only): two-click progressive reveal — click 1 =
   piece to move (violet), click 2 = destination (light violet), click 3 = hide.
   Disabled after deviation.
6. **Back to Setup** button: returns to the setup screen.
7. **Training mode toggle**: Drill (must play book move, revert on wrong) /
   Practice (free play with post-move feedback).
8. **Rating display**: shows current difficulty setting in the opening header area.

---

## 5. Opening Training Logic

### Data source
`assets/openings/{a–e}.tsv` — the lichess-org/chess-openings dataset, fetched
at runtime via `gloo-net` HTTP requests. Columns: `eco`, `name`, `pgn`.

### Parsing flow
1. At app start: the 5 TSV files are fetched asynchronously from `dist/assets/openings/`.
   A loading screen is shown while fetching. TSV rows are split into
   `OpeningEntry { eco, name, pgn }` (cheap string splitting, no chess logic).
2. At game start: the selected entry's PGN is parsed into
   `Vec<OpeningMove { from, to, promotion }>` by replaying moves through
   shakmaty's SAN parser. Fails gracefully (opening skipped) if PGN is invalid.

### Auto-play (computer's side)
- A reactive `Effect` watches `game` and `opening_move_idx`.
- When the game is in progress and it is the computer's turn with opening moves
  remaining, it schedules a 500 ms delayed move via `gloo_timers::TimeoutFuture`.
- After the delay the next opening move is applied and `opening_move_idx` incremented.

### Human's turn
- If the human's move matches `opening.moves[opening_move_idx]`, `opening_move_idx`
  is incremented. Play continues.
- If the human plays a different (but legal) move, `deviated = true`. The opening
  status shows "You deviated — free play from here." The computer stops auto-playing.
- If `opening_move_idx` reaches `opening.moves.len()`, the opening is complete.
  Status shows "Opening complete! Continue in free play."

### Hint
Pressing **Hint** sets `hint_active = true`. The board highlights:
- The piece to move: purple square.
- Its destination: lighter purple square.
Pressing Hint again hides the hint.

---

## 6. UI / Visual Requirements

- **Color palette**: retro warm-brown (LIGHT_SQ `#f0d9b5`, DARK_SQ `#b58863`,
  background `#1a0e0a`, accent `#c9a96e`).
- **Piece style**: Unicode glyphs — white outline (♔♕…), black filled (♚♛…).
- **Highlights**:
  - Selected piece square: yellow `#f6f669`
  - Reachable empty squares: green dot (rgba(0,128,0,0.5))
  - Reachable capture squares: green ring
  - King in check: red `#e63946`
  - Hint from-square: violet `#7b61ff`
  - Hint to-square: light violet `#a599ff`
- **Board labels**: rank numbers (1–8) on left, file letters (a–h) on bottom.
  Labels flip with board orientation.
- **Responsive**: squares sized `min(10vw, 64px)`.

---

## 9. Lichess Explorer API Integration

- **Endpoint**: `https://explorer.lichess.ovh/lichess?fen={FEN}&ratings={band}&speeds=rapid,classical`
- **Response**: `{ white, draws, black, moves: [{ uci, san, white, draws, black, averageRating }] }`
- **Rate limit**: sequential requests only, no auth, free
- **Hybrid seeded cache**:
  - Bundled JSON starter pack: top 50 openings, 6 ply deep (~200-400KB)
  - localStorage cache keyed by `"{FEN}|{rating_band}"`
  - Lookup order: localStorage → bundled → live API
  - API results written to localStorage on receipt (cache grows as user plays)
- **New module**: `explorer.rs`
- **New types**: `ExplorerMove`, `ExplorerResponse`, `ExplorerCache`

---

## 10. Rating Slider

- Range 800–2500 on setup screen, default 1600
- API band mapping: 800-1599 → 1600 band + randomness, 1600-1799 → 1600,
  1800-1999 → 1800, 2000-2199 → 2000, 2200-2499 → 2200, 2500 → 2500
- Sub-1600 randomness model: probability of picking a non-top move =
  `(1600 - selected) / 800`
- Passed from setup screen through app.rs to board.rs

---

## 11. Training Modes

- **Drill mode**: check book move *before* applying to game state. Wrong move →
  board unchanged, correct squares highlighted, "Try again" message.
- **Practice mode**: any legal move accepted. Post-move feedback:
  "You played {move}. Book move was {book_move}."
- Toggle on game screen, switchable mid-game
- Signal: `training_mode: RwSignal<TrainingMode>` (Drill | Practice)

---

## 12. Two-Click Hints

- Replace `hint_active: bool` with `hint_stage` enum:
  Hidden → SourceOnly → SourceAndTarget → Hidden
- SourceOnly: violet highlight on piece square only
- SourceAndTarget: also light violet on destination
- Disabled when deviated or opening complete
- Button label updates to reflect state

---

## 13. Opponent Variation

- Opponent uses Lichess Explorer frequency data for weighted random move selection
- Weight = total games (white + draws + black) for each candidate move
- May play common alternatives instead of exact book move
- Status: "Opponent played a common alternative: {SAN}. Free play from here."
- Falls back to deterministic book move if no Explorer data available

---

## 14. Black-Side Training

- Heuristic: openings with "Defense" in name, or where Black defines the system,
  tagged as Black-response
- When Black selected: sort Black-response openings to top of list
- Add "Black openings only" filter checkbox

---

## 16. Featured Opening Packs + Curated Request System

### Available Packs (playable)
Four curated "group" entries pinned at the top of the setup screen with a ★ star badge.

**Queen's Gambit Pack** (White)
- Matches `name.starts_with("Queen's Gambit")` or `name.starts_with("Slav Defense")` or `name.starts_with("Semi-Slav Defense")`.
- On game start: randomly selects one matching entry from the full loaded dataset.

**Modern Defense Pack** (Black)
- Matches `name.starts_with("Modern Defense")` or `name.starts_with("Pirc Defense")`.
- Covers lines vs 1.e4, 1.d4, 1.c4, 1.Nf3.

**Italian Game Pack** (White)
- Matches `name.starts_with("Italian Game")`.
- Covers Giuoco Piano, Evans Gambit, Two Knights, and more.

**Caro-Kann Defense Pack** (Black)
- Matches `name.starts_with("Caro-Kann Defense")`.
- Covers Advance, Classical, Exchange, Panov, and more.

### Curated Request List (10 groups)
Shown as autocomplete dropdown when user types ≥ 1 character. Static list in code:
1. ★ Queen's Gambit — Available
2. ★ Modern Defense — Available
3. Sicilian Defense — Requestable
4. Ruy Lopez — Requestable
5. French Defense — Requestable
6. ★ Caro-Kann Defense — Available
7. King's Indian Defense — Requestable
8. Nimzo-Indian Defense — Requestable
9. ★ Italian Game — Available
10. London System — Requestable

### Request System
- `FORMSPREE_ENDPOINT: &str` — fill in after registering at formspree.io
- `REQUESTS_OPEN: bool` — set `false` and rebuild to close the queue
- `MAX_REQUESTS_PER_WEEK: usize = 2` — per-user localStorage rolling window limit
- `SelectedEntry` enum: `Group(GroupKey) | Request(String)` (removed `Single(OpeningEntry)`)
- Rate limit state stored in `localStorage["chess_opening_requests"]` as `Vec<f64>` (JS timestamps)
- Formspree POST: `gloo_net::http::Request` (already a WASM dep) — cfg-gated with native no-op fallback

---

## 7. Success Criteria

### Already met (Phase 1 — Basic Chess)
- [x] Board renders with all 32 pieces in correct starting squares.
- [x] Clicking a piece highlights its legal moves (validated by shakmaty).
- [x] Self-check moves are rejected automatically.
- [x] Checkmate and Stalemate are detected and displayed.
- [x] No `unwrap()` calls — all failure paths use `Option`/`Result`.

### Already met (Phase 2 — Opening Trainer)
- [x] Setup screen allows choosing White or Black.
- [x] Board orientation flips when playing as Black.
- [x] Opening list loads all entries from the lichess dataset and is searchable.
- [x] Starting with an opening auto-plays the computer's side at each turn.
- [x] Hint button highlights the human's correct next move in the opening.
- [x] Deviation from opening is detected and reported in the status bar.
- [x] Opening complete message shown when all opening moves are exhausted.
- [x] "Back to Setup" resets the game and returns to the setup screen.

### Already met (Phase 3 — Performance)
- [x] Setup screen renders in < 500 ms on first load (max 50 rows shown initially).
- [x] Searching with 2+ characters shows up to 100 filtered results.
- [x] TSV opening data is NOT embedded in the WASM binary (fetched at runtime).
- [x] A loading indicator is shown while the TSV files are being fetched.
- [x] WASM binary size is reduced by ~370 KB vs Phase 2.
- [x] `gloo-net` is added to `Cargo.toml` for HTTP fetching.
- [x] `index.html` uses `data-trunk rel="copy-dir"` to copy TSV files to `dist/`.

### Phase 4 criteria — Explorer API + Rating + Opponent Variation
- [x] `to_fen()` returns valid FEN for any position
- [x] `explorer.rs` fetches from Lichess Explorer API
- [x] Responses cached in localStorage by (FEN, rating_band)
- [x] Bundled starter pack provides instant data for top 50 openings
- [x] Rating slider (800-2500) on setup screen, default 1600
- [x] Sub-1600 injects randomness into 1600-band data
- [x] Opponent plays weighted-random moves from Explorer data
- [x] App works offline with cached/bundled data (no crash without network)
- [x] All API requests sequential

### Phase 5 criteria — Featured Packs + Two-Click Hints + Undo (COMPLETE)
- [x] Four featured opening packs pinned at top of setup screen with ★
- [x] Queen's Gambit pack: randomly picks any QG variation (QGD, QGA, Slav, Semi-Slav, Exchange, Tarrasch) each game
- [x] Modern Defense pack: randomly picks from all Modern Defense lines (vs 1.e4/1.d4/1.c4/1.Nf3) with frequency weighting
- [x] Italian Game pack: randomly picks from all Italian Game variations (Giuoco Piano, Evans Gambit, Two Knights) each game
- [x] Caro-Kann Defense pack: randomly picks from all Caro-Kann Defense lines (Advance, Classical, Exchange)
- [x] Hint cycles 3 states: Off → Piece (from-square, dark violet) → Move (from+to, bright violet) → Off
- [x] ↩ Undo button with history stack + generation counter (cancels stale async tasks)
- [x] Action buttons moved above opening list
- [x] Hint/double-move bugs fixed (dual guards after each await in auto-play)
- [x] 36 Rust unit tests + 52 Playwright E2E tests

### Phase 8 criteria — Curated Opening Packs + Request System
- [ ] Remove 3,600+ opening browser from setup screen
- [ ] Search box hidden until ≥ 1 character typed (no default browse list)
- [ ] Autocomplete against curated list of 10 opening groups
- [ ] Available groups (QG, MD, Italian Game, Caro-Kann): clicking selects the featured pack
- [ ] Unavailable groups: clicking shows request panel with rate limit info
- [ ] "📩 Request something else" opens free-text request panel
- [ ] localStorage rate limit: 2 requests per user per 7-day rolling window
- [ ] Global queue control: `REQUESTS_OPEN: bool` constant (rebuild to toggle)
- [ ] Formspree POST on submission → email notification to developer
- [ ] Success/error/rate-limit/queue-closed status messages shown in UI
- [ ] Playwright E2E tests updated for new request UI

### Phase 6 criteria — Training Modes (future)
- [ ] Drill/Practice toggle on game screen
- [ ] Drill: wrong move doesn't change board, shows correct move highlighted
- [ ] Practice: free play with post-move feedback ("You played X. Book move was Y.")
- [ ] Toggle switchable mid-game

---

## 8. File Structure

```
simple-chess/
├── Cargo.toml
├── index.html
├── README.md
├── PRD.md
├── CLAUDE.md         Project-specific coding notes and known gotchas
├── serve.py          Gzip-capable static file server (trunk lacks compression)
├── assets/
│   ├── openings/
│   │   ├── a.tsv   (ECO group A — served at runtime, NOT embedded in WASM)
│   │   ├── b.tsv   (ECO group B)
│   │   ├── c.tsv   (ECO group C)
│   │   ├── d.tsv   (ECO group D)
│   │   └── e.tsv   (ECO group E)
│   └── explorer_cache.json   Bundled starter-pack data
└── src/
    ├── main.rs       WASM entry point
    ├── lib.rs        Module declarations
    ├── game.rs       Chess rules (shakmaty wrapper)
    ├── openings.rs   Opening data — async TSV fetch + PGN→moves
    ├── setup.rs      Setup screen component
    ├── board.rs      Game board component
    ├── explorer.rs   Lichess Explorer API client + cache
    └── app.rs        Root component + phase management
```
