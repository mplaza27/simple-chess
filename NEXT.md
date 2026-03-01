# NEXT.md — Handoff Prompt for Fresh Session

## Project Overview

Rust/Leptos 0.7/WASM chess opening trainer. Pure client-side app — no backend
server. Uses shakmaty for chess logic, Unicode pieces, retro brown palette.

## How to Get Oriented

1. **Read `CLAUDE.md`** first — build commands, shakmaty API gotchas, Leptos
   patterns, request system design, Playwright test notes.
2. **Read `PRD.md`** — full requirements. Phase 6 (training modes) is the next
   thing to build.
3. **Skim the source** in dependency order:
   - `src/game.rs` — pure chess logic (shakmaty wrapper, no UI imports)
   - `src/explorer.rs` — Lichess Explorer API client + 3-tier cache
   - `src/openings.rs` — async TSV fetch + PGN-to-moves parsing
   - `src/board.rs` — game board Leptos component
   - `src/setup.rs` — curated opening packs + request system UI
   - `src/app.rs` — root component, phase management (Setup <-> Game)

## Current State (after Phase 8 — Curated Packs + Request System)

- **Phases 1–5 + Phase 8 complete and working.**
  - Basic chess, opening trainer, performance, Explorer API, rating slider
  - ★ 4 featured packs (QG, MD, Italian Game, Caro-Kann), 3-state hint, ↩ Undo, generation counter
  - Double-move bug fixed (dual guards after each `await` in auto-play)
  - **Phase 8**: 3,600+ opening browser removed. Curated list of 10 opening
    groups (4 featured, 6 requestable) with search autocomplete. Request system
    via Formspree (rate-limited 2/week via localStorage). `SelectedEntry::Single` removed.
- **51 Playwright E2E tests pass** (`cd e2e && npx playwright test`)
  - `setup-screen.spec.ts` — 19 tests (load, color, slider, packs, curated search, request UI)
  - `navigation.spec.ts` — 7 tests (transitions, headers, round-trips)
  - `game-screen.spec.ts` — 15 tests (board, buttons, hint square highlights)
  - `openings.spec.ts` — 10 tests (QG + MD move sequences via featured packs, undo, deviation)
- **36 Rust unit tests pass** (`~/.cargo/bin/cargo test`)
- **Python venv** at `venv/` (created with `--without-pip`; stdlib-only scripts)
- **App runs** at `localhost:8080` via `python3 serve.py`
- Build: `~/.cargo/bin/trunk build --release`

## What to Build Next: Phase 6 — Training Modes

### PRD Section 11 — Training Modes
- **Drill mode**: check book move *before* applying to game state. Wrong move →
  board unchanged, correct squares highlighted, "Try again" message.
- **Practice mode**: any legal move accepted. Post-move feedback:
  "You played {move}. Book move was {book_move}."
- Toggle on game screen, switchable mid-game
- Signal: `training_mode: RwSignal<TrainingMode>` (Drill | Practice)

### Phase 6 criteria (from PRD)
- [ ] Drill/Practice toggle on game screen
- [ ] Drill: wrong move doesn't change board, shows correct move highlighted
- [ ] Practice: free play with post-move feedback ("You played X. Book move was Y.")
- [ ] Toggle switchable mid-game

## Key Architecture Notes

- **TSV loading still required**: `LocalResource::new(fetch_all_entries)` populates
  `all_entries_sv` which `start_opening()` uses to pick random variation from any
  featured pack (QG, MD, Italian Game, Caro-Kann). The visible list was removed but
  data loading stays.
- **`SelectedEntry` enum**: `Group(GroupKey) | Request(String)`.
  `Single(OpeningEntry)` was removed in Phase 8.
- **Rate limit helpers** are cfg-gated: WASM uses `gloo_storage::LocalStorage` + `js_sys::Date::now()`;
  native builds return `MAX_REQUESTS_PER_WEEK` / no-op (so `cargo test` still compiles).
- **Formspree submit** is cfg-gated: WASM uses `gloo_net`; native returns `Ok(())`.
- **`GroupKey` enum**: `QueensGambit | ModernDefense | ItalianGame | CaroKann`.
  `random_index(n)`: WASM uses `js_sys::Math::random()`, native returns 0.
- **Explorer 3-tier cache**: localStorage → `assets/explorer_cache.json` → live API.
- **`HintState` enum**: `Off | Piece | Full`. Cycles on click. Reset on each human move.
- **Generation counter** (`RwSignal<u32>`): increment on Undo/New Game to cancel stale
  async computer-move tasks.

## Key Constants (Phase 8 — set after deploying)

```rust
// src/setup.rs — read at compile time from env var
const FORMSPREE_ENDPOINT: Option<&str> = option_env!("FORMSPREE_ENDPOINT");
const REQUESTS_OPEN: bool = true;       // flip to false + rebuild to close queue
const MAX_REQUESTS_PER_WEEK: usize = 2; // per-user rolling 7-day limit
```
Set the env var before building: `FORMSPREE_ENDPOINT=https://formspree.io/f/YOUR_ID trunk build --release`.
In GitHub Actions, it's read from `secrets.FORMSPREE_ENDPOINT`. When unset, the request form shows "closed".

## Build / Test / Verify

```bash
# Run Rust unit tests (native, fast)
~/.cargo/bin/cargo test

# Build WASM (source .env to set FORMSPREE_ENDPOINT)
source .env && PATH="$PATH:$HOME/.cargo/bin" trunk build --release

# Serve locally
python3 serve.py
# Then open http://localhost:8080

# Run Playwright E2E tests (requires server running)
cd e2e && npx playwright test
npx playwright show-report   # after failures
```

## Explorer Cache (background task, not urgent)

The bundled `assets/explorer_cache.json` has only ~30 entries. Run when Lichess
rate limit has reset (wait ~1 hour after last heavy use):
```bash
source venv/bin/activate
python3 scripts/fetch_explorer_cache.py
```
Script uses 1.5 s delay + exponential backoff (5 s → 10 s → 20 s → 40 s) on 429s.
Takes ~15–20 min for full 288 calls.
