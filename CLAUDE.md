# CLAUDE.md — Project Notes for AI Assistants

Coding notes, known gotchas, and API quirks learned while building this project.

---

## Build & Toolchain

- **Build**: `source .env && ~/.cargo/bin/trunk build --release` (not `cargo build`). The `source .env` exports `FORMSPREE_ENDPOINT` so `option_env!()` picks it up at compile time.
- **Serve**: `python3 serve.py` — trunk has no built-in gzip; serve.py compresses WASM 1.2 MB → ~200 KB
- **Tests**: `~/.cargo/bin/cargo test` (runs natively, fast — no browser needed)
- **`.env` file**: Contains `FORMSPREE_ENDPOINT=...`. Gitignored — never commit secrets.
- **Trunk needs `cargo` in PATH**: if trunk can't find cargo, add `~/.cargo/bin` to PATH or set `CARGO` env var
- **Network access**: server must bind `0.0.0.0`, not `127.0.0.1`, to be reachable from other machines

## Python & Virtual Environment

- **Always use the project venv** for Python scripts to avoid system-level bloat.
- **Venv location**: `venv/` in project root (created with `python3 -m venv --without-pip venv`)
  - `python3-venv` package is NOT installed system-wide. Use `--without-pip` flag.
  - The venv has no pip; all scripts must use stdlib only (urllib, json, time, os).
- **Activate**: `source venv/bin/activate` before running scripts
- **Run scripts**: `venv/bin/python3 scripts/fetch_explorer_cache.py` (or activate first)
- **`venv/` is in `.gitignore`** — do not commit it
- **fetch_explorer_cache.py**: Uses only stdlib (urllib.request, json, time). No packages needed.
  Lichess Explorer rate-limits aggressively (~1 req/s). Script uses 1.5 s delay + exponential
  backoff (5 s → 10 s → 20 s → 40 s) on 429 errors. Run when rate limit has reset.

---

## shakmaty 0.30 API Gotchas

- `board.iter()` — NOT `board.pieces()` (doesn't exist in 0.30). Returns `(Square, Piece)` pairs.
- `position.play_unchecked(chess_move)` — takes `Move` **by value**, NOT by reference. Do NOT write `&chess_move`.
- `File::new(file_u8.into())` and `Rank::new(rank_u8.into())` — the argument must be `u32`, use `.into()` to coerce from `u8`.
- `San::from_ascii(token.as_bytes())` — takes `&[u8]`, convert with `.as_bytes()`.
- `san.to_move(&position)` — validates legality and returns concrete `Move`.
- `chess_move.from()` returns `Option<Square>` (for drop moves in variants); safe to unwrap for standard chess moves.

---

## Leptos 0.7 Patterns

- `#[derive(PartialEq)]` is **required** on any type used inside `Memo<T>`. Leptos compares old/new values to decide whether to rerun dependents.
- Doc comments with code blocks in Rust doc-tests: use ` ```text ` not ` ``` ` for ASCII art or non-Rust content, or the doc-test runner will try to compile it.
- `into_any()` is needed when two branches of a conditional return different concrete view types (type erasure to `AnyView`).
- `.get()` inside reactive closures (subscribes). `.get_untracked()` in event handlers (no subscription).
- `StoredValue::new(data)` — holds non-reactive data inside Leptos's ownership system so closures can access it.
- `collect_view()` — use instead of `.collect::<Vec<_>>()` when building a list of views.

## Auto-play Double-Move Bug (FIXED — do not regress)

**Root cause**: The auto-play `Effect` subscribes to multiple signals (`game`, `op_move_idx`, `deviated`). When they change in the same tick, the Effect fires multiple times, spawning duplicate `spawn_local` tasks with the same `generation` snapshot. The first task applies the computer's move. The second task then resumes after the async `lookup_explorer(...).await` — and without a second guard it applies another move on the human's turn.

**Fix**: Two guards in the async task:
1. Immediately after `TimeoutFuture::new(500).await` — checks generation and turn.
2. Immediately after `lookup_explorer(...).await` — re-checks generation and turn again.

Both `await` points are suspension points where the JS event loop can run other tasks. Either guard alone is insufficient. The pattern is:
```rust
// After each .await:
if generation_h.get_untracked() != gen_snapshot { return; }
let current = game_h.get_untracked();
if current.turn() == player_color { return; }
if current.outcome() != Outcome::InProgress { return; }
```

**Never remove either guard.** If the Explorer API is changed to non-async, remove guard 2; but guard 1 must always remain.

## Opening Dataset Edge Cases

- **Transpositions**: Many dataset entries reach a position via non-canonical move order (e.g. "QGD: Baltic Defense, Pseudo-Slav" plays Nf3 before c4). Do NOT assert fixed move-order sequences on swept dataset entries.
- **False name matches**: `name.contains("Queen's Gambit")` also matches "Zukertort Opening: Queen's Gambit Invitation" (1-move entry). Use `name.starts_with(...)` for family filtering.
- **False "Modern Defense" matches**: `name.contains("Modern Defense")` matches "Hungarian Opening: Reversed Modern Defense" (a White system). Use `name.starts_with("Modern Defense")`.
- **Test strategy**: Dataset sweeps → assert parse succeeds + count entries. Specific move-structure tests → use hardcoded PGNs.

---

## Performance Lessons

- **Never render all list items at startup.** Rendering 3,641 rows creates ~14,564 DOM nodes crossing the WASM↔JS bridge — the browser freezes. Cap with `BROWSE_LIMIT = 50` (no search) / `SEARCH_LIMIT = 100` (with search).
- **Don't embed large data with `include_str!()`** — 370 KB of TSV data inflates the WASM binary. Serve TSVs separately and fetch with `gloo-net` at runtime. Use `<link data-trunk rel="copy-dir" href="assets/openings" />` to copy them to `dist/`.
- **Trunk `copy-dir` strips the parent path**: `href="assets/openings"` copies to `dist/openings/`, NOT `dist/assets/openings/`. Fetch URLs must be `/openings/{file}.tsv`, not `/assets/openings/{file}.tsv`.
- **`gloo-net` is WASM-only**: add it under `[target.'cfg(target_arch = "wasm32")'.dependencies]` so `cargo test` (native) still compiles. Gate the `use gloo_net::...` import with `#[cfg(target_arch = "wasm32")]`.
- **Dual-cfg async fn for native+WASM**: provide a `#[cfg(not(target_arch = "wasm32"))]` fallback using `include_str!` so native tests don't need a server. The `async fn` resolves immediately on native since `include_str!` is synchronous.
- **`LocalResource::new` awaiting**: inside `Suspend::new`, `resource.await` returns the resolved data. Use `.iter().cloned().collect::<Vec<_>>()` to get an owned `Vec` regardless of the exact wrapper type Leptos uses.
- **Async loading pattern in Leptos 0.7**: `LocalResource::new(|| fetch_fn())` + `<Suspense fallback=...>` + `Suspend::new(async move { ... })`.

---

## Opening Request System (Phase 8)

The 3,600+ opening browser was removed. `setup.rs` now has a curated list of 10 groups
(4 featured packs: QG, MD, Italian Game, Caro-Kann; 6 requestable) and a Formspree-backed request form.

### Key constants (top of `setup.rs`):
- `FORMSPREE_ENDPOINT`: read at compile time via `option_env!("FORMSPREE_ENDPOINT")`. Set as env var
  before building (`export FORMSPREE_ENDPOINT=https://formspree.io/f/YOUR_ID`). When unset, the
  request system auto-disables (queue appears closed). In CI, set as a GitHub Actions secret.
- `REQUESTS_OPEN: bool = true` — set `false` + rebuild to close the queue.
- `MAX_REQUESTS_PER_WEEK: usize = 2` — per-user rolling 7-day limit.

### localStorage rate limiting (WASM-only):
```rust
// Key: "chess_opening_requests"  Value: Vec<f64> (JS Date.now() timestamps)
// Filter to timestamps within the last week_ms = 7 * 24 * 60 * 60 * 1000.0
```
Use `gloo_storage::LocalStorage::{get, set}` (already a WASM dep from `explorer.rs`).
Native cfg fallback: `requests_remaining()` returns `MAX_REQUESTS_PER_WEEK`, `record_request()` is a no-op.

### Formspree submission (WASM-only):
Use `gloo_net::http::Request::post(FORMSPREE_ENDPOINT)` with `Content-Type: application/json`
and `Accept: application/json`. Native cfg fallback: `submit_opening_request()` returns `Ok(())`.

### SelectedEntry enum (updated):
```rust
pub enum SelectedEntry {
    Group(GroupKey),    // Available pack → "▶ Start Opening" enabled
    Request(String),    // Unavailable opening name → request panel shown
}
```
`Single(OpeningEntry)` was removed — no individual line selection anymore.

### TSV loading still required:
Keep `LocalResource::new(fetch_all_entries)` + `Suspense` to populate `all_entries_sv`
for the QG/MD random variation picker. Just hide the visible list from the Suspend view.

---

## Playwright E2E Tests

Suite lives in `e2e/` (separate npm project, not in Rust workspace).

```bash
# Prerequisites: server must be running at localhost:8080
python3 serve.py &
cd e2e && npx playwright test
npx playwright show-report     # after failures
```

Key constraints:
- No CSS classes/IDs anywhere — selectors use Unicode button text, ARIA roles, structural locators
- Featured pack cards are `<div>` not `<button>` — use `page.getByText(...)` to click
- `↩ Undo` uses `opacity:0.4` in style (not `disabled` attr) — check `toHaveAttribute('style', /opacity:0\.4/)`
- Two-phase WASM load: heading appears first, then TSV data loads — `gotoSetup()` waits for hidden `[data-loaded="true"]` marker
- No visible opening list — data loads invisibly for QG/MD random picker only
- Auto-play delay is 500 ms + optional Explorer HTTP — use 8 s timeout on turn-change assertions
- `boardCell(page, sq)` helper in `helpers.ts` converts algebraic notation to grid cell locator

---

## Project Architecture

```
game.rs (pure logic, no UI)
  ↑ called by
board.rs (Leptos UI component)
  ↑ mounted by
app.rs (phase manager: Setup ↔ Game)

openings.rs (async TSV fetch + PGN parsing)
  ↑ used by
setup.rs (opening selector UI + curated request form)
  ↑ called back from
app.rs
```

- Keep game.rs free of any UI imports. Tests run natively against it.
- All signals live in the component that owns them. Pass callbacks down with `impl Fn(...) + 'static`.
