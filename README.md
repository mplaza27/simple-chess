# Simple Chess

A web-based chess app built with **Rust**, **Leptos 0.7**, and **shakmaty**.
Written as a learning project — every file is heavily commented to explain
the Rust and Leptos concepts as they appear.

---

## Quick Start

```bash
# 1. Build the optimized release bundle (run once, or after code changes)
~/.cargo/bin/trunk build --release

# 2. Serve with gzip compression (browsers download ~200 KB instead of 1.2 MB)
python3 serve.py

# 3. Open in your browser
http://<server-ip>:8080

# 4. Run the unit tests (fast — no browser needed)
~/.cargo/bin/cargo test
```

> **Why `serve.py` instead of `trunk serve`?**
> `trunk serve` doesn't compress responses. The WASM binary embeds 3,600+
> chess openings (~400 KB of text), making the uncompressed download 1.2 MB.
> `serve.py` adds gzip, shrinking it to ~200 KB — much faster over a
> Tailscale or SSH-forwarded connection.
>
> For local-only development, `trunk serve` still works fine:
> ```bash
> ~/.cargo/bin/trunk serve --port 8080 --address 0.0.0.0 --no-autoreload
> ```

---

## How to Play

1. Click any of **White's pieces** to select it.
2. Green dots appear on every **legal destination** — shakmaty guarantees
   these are all valid (no self-checks possible).
3. Click a green dot to **move** the piece.
4. Repeat for Black, then White, alternating turns.
5. The status bar announces **CHECK**, **Checkmate**, or **Stalemate**.
6. Click **New Game** to reset.

---

## Project Structure

```
simple-chess/
├── Cargo.toml          ← Project manifest (dependencies, build settings)
├── index.html          ← HTML entry point — Trunk injects the WASM bundle here
└── src/
    ├── main.rs         ← WASM entry point: mounts the app to <body>
    ├── lib.rs          ← Library root: declares the three modules
    ├── game.rs         ← Chess logic (no UI) — wraps shakmaty
    ├── board.rs        ← Leptos board component (all UI code lives here)
    └── app.rs          ← Root Leptos component
```

### The separation of concerns

```
           ┌──────────────────────────────────────┐
           │            Browser / DOM              │
           └──────────────────┬───────────────────┘
                              │ Leptos renders
           ┌──────────────────▼───────────────────┐
           │   board.rs — UI Layer                 │
           │                                       │
           │  RwSignal<GameState>  ←── signals     │
           │  RwSignal<Option<Square>>             │
           │  RwSignal<Vec<Square>>                │
           │  RwSignal<String>                     │
           └──────────────────┬───────────────────┘
                              │ calls methods on
           ┌──────────────────▼───────────────────┐
           │   game.rs — Logic Layer               │
           │                                       │
           │  GameState { position: Chess }        │
           │  .legal_moves_from(sq)                │
           │  .apply_move(from, to)                │
           │  .outcome()                           │
           └──────────────────┬───────────────────┘
                              │ delegates rules to
           ┌──────────────────▼───────────────────┐
           │   shakmaty (external crate)           │
           │   — legal move generation             │
           │   — check / checkmate / stalemate     │
           │   — FEN parsing                       │
           └──────────────────────────────────────┘
```

---

## Key Rust Concepts in This Project

### Ownership & Borrowing
Rust's defining feature. Every value has one *owner*. You can lend it out
temporarily with *borrows* (`&T` = read-only, `&mut T` = read-write), but
the compiler guarantees you never have two mutable borrows at once, and
nothing is ever used after it's freed. No garbage collector needed.

In this project:
- `&self` on `GameState` methods = read-only borrow (just looking at the position)
- `&mut self` on `apply_move` = exclusive mutable borrow (changing the position)

### `Option<T>`
Rust has no `null`. Instead, a value that might not exist is `Option<T>`:
- `Some(value)` — it exists
- `None` — it doesn't

`piece_at(square)` returns `Option<(Color, Role)>` because a square might be empty.

### Enums with data
`Outcome::Checkmate { winner: Color }` carries data inside the variant.
This makes illegal states unrepresentable — you can't have a `Checkmate`
without knowing who won.

### Iterators
Rust iterators are *lazy pipelines*:
```rust
legal_moves()
    .into_iter()
    .filter(|m| m.from() == Some(sq))  // keep matching moves
    .map(|m| m.to())                   // extract destinations
    .collect()                          // gather into Vec
```
Nothing runs until `.collect()` is called. Very memory-efficient.

### Closures & `move`
A closure is an anonymous function: `|arg| expression`.
`move` closures take *ownership* of captured variables, needed when the
closure outlives the scope that created it (common in event handlers).

### `#[derive(...)]`
Automatically generates trait implementations:
```rust
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome { ... }
```
- `Clone` → `.clone()` works
- `Debug` → `{:?}` printing works
- `PartialEq` → `==` works (used in tests)

---

## Key Leptos Concepts in This Project

### `RwSignal<T>`
A reactive container. When `.set()` or `.update()` is called, any code
that previously read it with `.get()` is automatically re-run.

```rust
let selected = RwSignal::new(Option::<Square>::None);

// In an event handler:
selected.set(Some(square));  // triggers re-render

// In a view closure:
let sq = selected.get();     // subscribes this closure to `selected`
```

### `#[component]`
Marks a function as a Leptos component. The function must return
`impl IntoView`. Components can be used in `view!` macros as tags:
`<ChessBoard />`.

### `view!` macro
Leptos's JSX-like template syntax:
```rust
view! {
    <div class="board">
        <span>{move || some_signal.get()}</span>
    </div>
}
```
Closures inside `{}` are reactive — Leptos calls them again when their
signals change.

### `.get()` vs `.get_untracked()`
- `.get()` — reads the value AND registers the calling code as a dependent.
  Used inside reactive closures (like `squares_view`) so they re-run on changes.
- `.get_untracked()` — reads without registering. Used in event handlers,
  where you just want the current value without setting up a subscription.

---

## Running the Tests

```bash
~/.cargo/bin/cargo test
```

Tests are in `src/game.rs` in a `mod tests` block marked `#[cfg(test)]`.
They run natively (not in WASM), so they're fast. Each test is decorated
with `#[test]` and uses `assert!` / `assert_eq!` macros to check results.

---

## Generating API Documentation

```bash
~/.cargo/bin/cargo doc --open
```

This builds HTML documentation from all `///` and `//!` comments in the
source and opens it in your browser. Every public item is documented.

---
