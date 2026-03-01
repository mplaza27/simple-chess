//! # main.rs — WASM entry point
//!
//! This is the first thing that runs when the browser loads the app.
//! It is compiled as a `[[bin]]` target (see `Cargo.toml`) so it produces
//! an executable — in this case, a WASM module.
//!
//! ## Why is this separate from `lib.rs`?
//!
//! Rust projects can have both a `lib` (library) and a `bin` (binary) in the
//! same crate. Keeping them separate means:
//!
//! - `lib.rs` (and everything it declares) can be compiled natively for
//!   `cargo test` — fast, no browser needed.
//!
//! - `main.rs` (the WASM entry point) is compiled to WASM only by Trunk.
//!
//! The binary depends on the library: `use simple_chess::app::App;` pulls in
//! the `App` component from `lib.rs`.
//!
//! ## `mount_to_body`
//!
//! This Leptos function takes a component function, instantiates it, and
//! appends the resulting DOM nodes to `document.body`. From that point on,
//! Leptos owns the UI and updates it reactively in response to signal changes.

use leptos::prelude::*;
use simple_chess::app::App;

fn main() {
    // `mount_to_body` is the "start the engine" call.
    // It renders the `App` component into the browser's <body> element.
    // Everything reactive (signals, event listeners) is set up here.
    mount_to_body(App);
}
