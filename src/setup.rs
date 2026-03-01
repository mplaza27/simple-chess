//! # setup.rs — Opening Selection Screen (Phase 8)
//!
//! The first thing the user sees. Lets them choose:
//! 1. Which color to play (White or Black)
//! 2. Opponent rating level (800–2500) for Explorer move weighting
//! 3. A Featured Opening Pack (Queen's Gambit or Modern Defense) — randomly
//!    selects a variation each game
//!
//! ## Curated Request System
//! Users can search a curated list of 10 well-known opening groups.
//! Available packs (QG, MD) can be selected directly. Unavailable openings
//! can be requested via Formspree email submission, rate-limited to 2/week
//! per user via localStorage timestamps.
//!
//! ## Async loading pattern (Leptos 0.7)
//! Opening data is still fetched at runtime (needed for QG/MD random variation
//! picker) but the visible 3,600+ list is removed.

use leptos::prelude::*;
use leptos::task::spawn_local;
use shakmaty::Color;

use crate::openings::{fetch_all_entries, Opening, OpeningEntry, parse_opening};

#[cfg(target_arch = "wasm32")]
use js_sys;

#[cfg(target_arch = "wasm32")]
use gloo_storage::{LocalStorage, Storage};

// ── Visual constants ────────────────────────────────────────────────────────
const BG: &str       = "#1a0e0a";
const PANEL: &str    = "#2a1a12";
const BORDER: &str   = "#6b3a2a";
const ACCENT: &str   = "#c9a96e";
const TEXT: &str     = "#f0d9b5";
const BTN_BG: &str   = "#6b3a2a";
const SELECTED: &str = "#8b4a1a";
const MUTED: &str    = "#8a7a6a";

// ── Request system constants ────────────────────────────────────────────────
/// Formspree endpoint, read at compile time from the `FORMSPREE_ENDPOINT` env var.
/// When unset, the request system is automatically disabled (queue appears closed).
const FORMSPREE_ENDPOINT: Option<&str> = option_env!("FORMSPREE_ENDPOINT");
const REQUESTS_OPEN: bool = true;
const MAX_REQUESTS_PER_WEEK: usize = 2;

// ── Curated opening list ────────────────────────────────────────────────────

struct CuratedOpening {
    name: &'static str,
    description: &'static str,
    group_key: Option<GroupKey>,
}

const CURATED_OPENINGS: &[CuratedOpening] = &[
    CuratedOpening { name: "Queen's Gambit",      description: "QGD · QGA · Slav · Semi-Slav · Exchange · Tarrasch",     group_key: Some(GroupKey::QueensGambit) },
    CuratedOpening { name: "Modern / Pirc Defense", description: "Modern ...g6 · Pirc ...d6 · vs 1.e4 · vs 1.d4",        group_key: Some(GroupKey::ModernDefense) },
    CuratedOpening { name: "Sicilian Defense",    description: "Open · Najdorf · Dragon · Scheveningen",                group_key: None },
    CuratedOpening { name: "Ruy Lopez",           description: "Berlin · Morphy · Closed · Exchange",                   group_key: None },
    CuratedOpening { name: "French Defense",      description: "Winawer · Tarrasch · Advance · Classical",              group_key: None },
    CuratedOpening { name: "Caro-Kann Defense",   description: "Advance · Classical · Exchange",                        group_key: Some(GroupKey::CaroKann) },
    CuratedOpening { name: "King's Indian Defense", description: "Classical · Sämisch · Four Pawns",                    group_key: None },
    CuratedOpening { name: "Nimzo-Indian Defense", description: "Rubinstein · Classical · Hübner",                      group_key: None },
    CuratedOpening { name: "Italian Game",        description: "Giuoco Piano · Evans Gambit · Two Knights",             group_key: Some(GroupKey::ItalianGame) },
    CuratedOpening { name: "London System",       description: "Bf4 systems · Anti-everything repertoire",              group_key: None },
];

// ── Opening Group types ─────────────────────────────────────────────────────

/// Which featured opening pack is selected.
#[derive(Clone, Debug, PartialEq)]
pub enum GroupKey {
    QueensGambit,
    ModernDefense,
    ItalianGame,
    CaroKann,
}

/// What is currently selected.
#[derive(Clone, Debug, PartialEq)]
pub enum SelectedEntry {
    /// A featured pack — randomly picks a matching line on Start.
    Group(GroupKey),
    /// An unavailable opening name — request panel shown.
    Request(String),
}

/// Status of a request submission.
#[derive(Clone, Debug, PartialEq)]
enum RequestStatus {
    Idle,
    Submitting,
    Success,
    RateLimited,
    QueueClosed,
    Error(String),
}

/// Returns true if an entry belongs to the Queen's Gambit family.
pub(crate) fn is_queens_gambit(e: &OpeningEntry) -> bool {
    e.name.starts_with("Queen's Gambit")
        || e.name.starts_with("Slav Defense")
        || e.name.starts_with("Semi-Slav Defense")
}

/// Returns true if an entry belongs to the Modern Defense / Pirc Defense family.
pub(crate) fn is_modern_defense(e: &OpeningEntry) -> bool {
    e.name.starts_with("Modern Defense") || e.name.starts_with("Pirc Defense")
}

/// Returns true if an entry belongs to the Italian Game family.
pub(crate) fn is_italian_game(e: &OpeningEntry) -> bool {
    e.name.starts_with("Italian Game")
}

/// Returns true if an entry belongs to the Caro-Kann Defense family.
pub(crate) fn is_caro_kann(e: &OpeningEntry) -> bool {
    e.name.starts_with("Caro-Kann Defense")
}

/// Returns the player color for a given opening group.
/// QG = White (1.d4 systems), MD = Black (hypermodern ...d6/g6 systems).
pub(crate) fn color_for_group(key: &GroupKey) -> Color {
    match key {
        GroupKey::QueensGambit => Color::White,
        GroupKey::ModernDefense => Color::Black,
        GroupKey::ItalianGame => Color::White,
        GroupKey::CaroKann => Color::Black,
    }
}

/// Random index in [0, n). WASM uses Math.random(); native returns 0.
pub(crate) fn random_index(n: usize) -> usize {
    if n == 0 { return 0; }
    #[cfg(target_arch = "wasm32")]
    { (js_sys::Math::random() * n as f64) as usize }
    #[cfg(not(target_arch = "wasm32"))]
    { 0 }
}

// ── Rate-limit helpers (cfg-gated) ──────────────────────────────────────────

#[cfg(target_arch = "wasm32")]
fn requests_remaining() -> usize {
    let timestamps: Vec<f64> = LocalStorage::get("chess_opening_requests").unwrap_or_default();
    let week_ms = 7.0 * 24.0 * 60.0 * 60.0 * 1000.0;
    let now = js_sys::Date::now();
    let recent = timestamps.iter().filter(|&&t| now - t < week_ms).count();
    MAX_REQUESTS_PER_WEEK.saturating_sub(recent)
}

#[cfg(not(target_arch = "wasm32"))]
fn requests_remaining() -> usize {
    MAX_REQUESTS_PER_WEEK
}

#[cfg(target_arch = "wasm32")]
fn record_request() {
    let mut timestamps: Vec<f64> = LocalStorage::get("chess_opening_requests").unwrap_or_default();
    timestamps.push(js_sys::Date::now());
    let _ = LocalStorage::set("chess_opening_requests", &timestamps);
}

#[cfg(not(target_arch = "wasm32"))]
fn record_request() {}

// ── Formspree submission (cfg-gated) ────────────────────────────────────────

#[cfg(target_arch = "wasm32")]
async fn submit_opening_request(name: String) -> Result<(), String> {
    let endpoint = FORMSPREE_ENDPOINT
        .ok_or_else(|| "Request system not configured".to_string())?;
    let body = serde_json::json!({ "opening": name });
    let resp = gloo_net::http::Request::post(endpoint)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(body.to_string())
        .map_err(|e| format!("{e}"))?
        .send()
        .await
        .map_err(|e| format!("{e}"))?;
    if resp.ok() { Ok(()) } else { Err(format!("HTTP {}", resp.status())) }
}

#[cfg(not(target_arch = "wasm32"))]
async fn submit_opening_request(_name: String) -> Result<(), String> {
    Ok(())
}

// ── SetupScreen component ───────────────────────────────────────────────────

/// The setup / opening selection screen.
///
/// ## Props
/// - `on_start`: called with `(color, opening, rating, matching_entries)` to begin an opening game.
#[component]
pub fn SetupScreen(
    on_start: impl Fn(Color, Opening, u16, Vec<OpeningEntry>, usize) + 'static,
) -> impl IntoView {

    // ── Async load (still needed for QG/MD random variation picker) ──────────
    let entries_resource = LocalResource::new(fetch_all_entries);

    // StoredValue populated by Suspend block — holds full dataset for start_opening().
    let all_entries_sv = StoredValue::new(Vec::<OpeningEntry>::new());

    // ── Reactive signals ────────────────────────────────────────────────────
    let search_text    = RwSignal::new(String::new());
    let selected_entry = RwSignal::new(Option::<SelectedEntry>::None);
    let rating         = RwSignal::new(1600u16);
    let request_status = RwSignal::new(RequestStatus::Idle);
    let show_custom    = RwSignal::new(false);
    let custom_text    = RwSignal::new(String::new());

    // ── Action handlers ─────────────────────────────────────────────────────
    let start_opening = move || {
        let r     = rating.get_untracked();

        match selected_entry.get_untracked() {
            Some(SelectedEntry::Group(key)) => {
                let color = color_for_group(&key);
                let matching: Vec<OpeningEntry> = all_entries_sv.with_value(|entries| {
                    entries.iter().filter(|e| match key {
                        GroupKey::QueensGambit  => is_queens_gambit(e),
                        GroupKey::ModernDefense => is_modern_defense(e),
                        GroupKey::ItalianGame   => is_italian_game(e),
                        GroupKey::CaroKann      => is_caro_kann(e),
                    }).cloned().collect()
                });
                if matching.is_empty() { return; }
                let idx   = random_index(matching.len());
                let entry = &matching[idx];
                if let Some(opening) = parse_opening(entry) {
                    on_start(color, opening, r, matching, idx);
                }
            }
            _ => {}
        }
    };

    // ── Request submission handler ──────────────────────────────────────────
    let do_submit_request = move |name: String| {
        if !REQUESTS_OPEN || FORMSPREE_ENDPOINT.is_none() {
            request_status.set(RequestStatus::QueueClosed);
            return;
        }
        if requests_remaining() == 0 {
            request_status.set(RequestStatus::RateLimited);
            return;
        }
        request_status.set(RequestStatus::Submitting);
        spawn_local(async move {
            match submit_opening_request(name).await {
                Ok(()) => {
                    record_request();
                    request_status.set(RequestStatus::Success);
                }
                Err(msg) => {
                    request_status.set(RequestStatus::Error(msg));
                }
            }
        });
    };

    // ── View ────────────────────────────────────────────────────────────────
    view! {
        <div style=format!(
            "display:flex; flex-direction:column; align-items:center; \
             font-family:'Courier New', monospace; background:{BG}; \
             min-height:100vh; padding:2rem; gap:1.5rem; color:{TEXT};"
        )>

            // ── Title ───────────────────────────────────────────────────────
            <h1 style=format!("color:{ACCENT}; letter-spacing:0.15em; font-size:1.6rem; \
                               text-transform:uppercase; margin:0;")>
                "♟ Simple Chess ♟"
            </h1>
            <p style=format!("color:{ACCENT}; font-size:0.85rem; margin:0; opacity:0.8;")>
                "Opening Trainer"
            </p>

            // ── Rating slider ───────────────────────────────────────────────
            <div style=format!("display:flex; flex-direction:column; align-items:center; \
                                gap:0.5rem; padding:1rem 1.5rem; border:1px solid {BORDER}; \
                                background:{PANEL}; width:100%; max-width:560px;")>
                <span style=format!("color:{ACCENT}; font-size:0.8rem; letter-spacing:0.1em; \
                                     text-transform:uppercase;")>
                    "Opponent Rating"
                </span>
                <div style="display:flex; align-items:center; gap:1rem; width:100%;">
                    <span style=format!("color:{ACCENT}; font-size:0.75rem; opacity:0.6;")>
                        "800"
                    </span>
                    <input
                        type="range"
                        min="800"
                        max="2500"
                        step="100"
                        prop:value=move || rating.get().to_string()
                        style=format!("flex:1; accent-color:{ACCENT};")
                        on:input=move |e| {
                            if let Ok(v) = event_target_value(&e).parse::<u16>() {
                                rating.set(v);
                            }
                        }
                    />
                    <span style=format!("color:{ACCENT}; font-size:0.75rem; opacity:0.6;")>
                        "2500"
                    </span>
                </div>
                <span style=format!("color:{TEXT}; font-size:0.9rem; font-weight:bold;")>
                    {move || format!("{}", rating.get())}
                </span>
            </div>

            // ── Featured Opening Packs ──────────────────────────────────────
            <div style=format!("display:flex; flex-direction:column; align-items:center; \
                                gap:0.5rem; padding:1rem 1.5rem; border:1px solid {BORDER}; \
                                background:{PANEL}; width:100%; max-width:560px;")>
                <span style=format!("color:{ACCENT}; font-size:0.8rem; letter-spacing:0.1em; \
                                     text-transform:uppercase;")>
                    "★ Featured Opening Packs"
                </span>
                <div style="display:flex; gap:0.75rem; flex-wrap:wrap; justify-content:center;">

                    // Queen's Gambit pack
                    <div
                        style=move || featured_card_style(
                            selected_entry.get() == Some(SelectedEntry::Group(GroupKey::QueensGambit))
                        )
                        on:click=move |_| {
                            selected_entry.set(Some(SelectedEntry::Group(GroupKey::QueensGambit)));
                            request_status.set(RequestStatus::Idle);
                            show_custom.set(false);
                        }
                    >
                        <span style=format!("color:{ACCENT}; font-size:0.9rem; \
                                             font-weight:bold; letter-spacing:0.05em;")>
                            "★ Queen's Gambit"
                        </span>
                        <span style=format!("color:{TEXT}; font-size:0.7rem; opacity:0.8; \
                                             margin-top:0.2rem;")>
                            "QGD · QGA · Slav · Semi-Slav"
                        </span>
                        <span style=format!("color:{TEXT}; font-size:0.7rem; opacity:0.7;")>
                            "Exchange · Tarrasch · and more"
                        </span>
                        <span style=format!("color:{ACCENT}; font-size:0.65rem; opacity:0.6; \
                                             margin-top:0.3rem;")>
                            "Play as ♔ White"
                        </span>
                    </div>

                    // Modern Defense pack
                    <div
                        style=move || featured_card_style(
                            selected_entry.get() == Some(SelectedEntry::Group(GroupKey::ModernDefense))
                        )
                        on:click=move |_| {
                            selected_entry.set(Some(SelectedEntry::Group(GroupKey::ModernDefense)));
                            request_status.set(RequestStatus::Idle);
                            show_custom.set(false);
                        }
                    >
                        <span style=format!("color:{ACCENT}; font-size:0.9rem; \
                                             font-weight:bold; letter-spacing:0.05em;")>
                            "★ Modern / Pirc Defense"
                        </span>
                        <span style=format!("color:{TEXT}; font-size:0.7rem; opacity:0.8; \
                                             margin-top:0.2rem;")>
                            "Modern ...g6 · Pirc ...d6"
                        </span>
                        <span style=format!("color:{TEXT}; font-size:0.7rem; opacity:0.7;")>
                            "vs 1.e4 · vs 1.d4 · and more"
                        </span>
                        <span style=format!("color:{ACCENT}; font-size:0.65rem; opacity:0.6; \
                                             margin-top:0.3rem;")>
                            "Play as ♚ Black"
                        </span>
                    </div>

                    // Italian Game pack
                    <div
                        style=move || featured_card_style(
                            selected_entry.get() == Some(SelectedEntry::Group(GroupKey::ItalianGame))
                        )
                        on:click=move |_| {
                            selected_entry.set(Some(SelectedEntry::Group(GroupKey::ItalianGame)));
                            request_status.set(RequestStatus::Idle);
                            show_custom.set(false);
                        }
                    >
                        <span style=format!("color:{ACCENT}; font-size:0.9rem; \
                                             font-weight:bold; letter-spacing:0.05em;")>
                            "★ Italian Game"
                        </span>
                        <span style=format!("color:{TEXT}; font-size:0.7rem; opacity:0.8; \
                                             margin-top:0.2rem;")>
                            "Giuoco Piano · Evans Gambit"
                        </span>
                        <span style=format!("color:{TEXT}; font-size:0.7rem; opacity:0.7;")>
                            "Two Knights · and more"
                        </span>
                        <span style=format!("color:{ACCENT}; font-size:0.65rem; opacity:0.6; \
                                             margin-top:0.3rem;")>
                            "Play as ♔ White"
                        </span>
                    </div>

                    // Caro-Kann Defense pack
                    <div
                        style=move || featured_card_style(
                            selected_entry.get() == Some(SelectedEntry::Group(GroupKey::CaroKann))
                        )
                        on:click=move |_| {
                            selected_entry.set(Some(SelectedEntry::Group(GroupKey::CaroKann)));
                            request_status.set(RequestStatus::Idle);
                            show_custom.set(false);
                        }
                    >
                        <span style=format!("color:{ACCENT}; font-size:0.9rem; \
                                             font-weight:bold; letter-spacing:0.05em;")>
                            "★ Caro-Kann Defense"
                        </span>
                        <span style=format!("color:{TEXT}; font-size:0.7rem; opacity:0.8; \
                                             margin-top:0.2rem;")>
                            "Advance · Classical · Exchange"
                        </span>
                        <span style=format!("color:{TEXT}; font-size:0.7rem; opacity:0.7;")>
                            "Panov · and more"
                        </span>
                        <span style=format!("color:{ACCENT}; font-size:0.65rem; opacity:0.6; \
                                             margin-top:0.3rem;")>
                            "Play as ♚ Black"
                        </span>
                    </div>

                </div>
            </div>

            // ── Action buttons ──────────────────────────────────────────────
            <div style="display:flex; gap:1rem;">
                <button
                    style=move || action_btn_style(
                        matches!(selected_entry.get(), Some(SelectedEntry::Group(_)))
                    )
                    disabled=move || !matches!(selected_entry.get(), Some(SelectedEntry::Group(_)))
                    on:click=move |_| start_opening()
                >
                    "▶ Start Opening"
                </button>
            </div>

            // ── Invisible data loader (TSV still needed for QG/MD picker) ───
            <Suspense fallback=move || view! {
                <div style=format!(
                    "color:{ACCENT}; font-size:0.85rem; padding:0.5rem;"
                )>
                    "⏳ Loading opening data…"
                </div>
            }>
                {move || Suspend::new(async move {
                    let loaded = entries_resource.await;
                    all_entries_sv.set_value(loaded.iter().cloned().collect::<Vec<_>>());
                    view! { <span style="display:none;" data-loaded="true" /> }
                })}
            </Suspense>

            // ── Request an Opening section ──────────────────────────────────
            <div style=format!("display:flex; flex-direction:column; gap:0.5rem; \
                                width:100%; max-width:560px;")>
                <span style=format!("color:{ACCENT}; font-size:0.8rem; letter-spacing:0.1em; \
                                     text-transform:uppercase;")>
                    "Request an Opening"
                </span>

                // Search input
                <input
                    type="text"
                    placeholder="Search openings…"
                    prop:value=move || search_text.get()
                    style=format!("background:{PANEL}; color:{TEXT}; border:1px solid {BORDER}; \
                                   padding:0.4rem 0.8rem; font-family:'Courier New', monospace; \
                                   font-size:0.85rem; width:100%; box-sizing:border-box;")
                    on:input=move |e| {
                        let val = event_target_value(&e);
                        search_text.set(val);
                        selected_entry.set(None);
                        show_custom.set(false);
                        request_status.set(RequestStatus::Idle);
                    }
                />

                // ── Autocomplete dropdown (visible only when search ≥ 1 char) ─
                {move || {
                    let query = search_text.get().to_lowercase();
                    if query.is_empty() {
                        return view! { <div style="display:none;" /> }.into_any();
                    }

                    let matches: Vec<(usize, &'static CuratedOpening)> = CURATED_OPENINGS.iter()
                        .enumerate()
                        .filter(|(_, c)| c.name.to_lowercase().contains(&query))
                        .collect();

                    view! {
                        <div style=format!(
                            "border:1px solid {BORDER}; background:{PANEL}; \
                             font-size:0.8rem; max-height:280px; overflow-y:auto;"
                        )>
                            {matches.into_iter().map(|(_idx, curated)| {
                                let name = curated.name;
                                let _desc = curated.description;
                                let is_available = curated.group_key.is_some();
                                let group_key = curated.group_key.clone();

                                view! {
                                    <div
                                        style=format!(
                                            "display:flex; align-items:center; padding:0.4rem 0.8rem; \
                                             gap:0.5rem; cursor:pointer; \
                                             border-bottom:1px solid rgba(107,58,42,0.3);"
                                        )
                                        on:click=move |_| {
                                            if let Some(ref key) = group_key {
                                                selected_entry.set(Some(SelectedEntry::Group(key.clone())));
                                            } else {
                                                selected_entry.set(Some(SelectedEntry::Request(name.to_string())));
                                            }
                                            search_text.set(String::new());
                                            show_custom.set(false);
                                            request_status.set(RequestStatus::Idle);
                                        }
                                    >
                                        <span style=format!("color:{}; flex:1;",
                                            if is_available { ACCENT } else { TEXT })>
                                            {if is_available { "★ " } else { "" }}
                                            {name}
                                        </span>
                                        <span style=format!("font-size:0.7rem; color:{};",
                                            if is_available { ACCENT } else { MUTED })>
                                            {if is_available { "Available" } else { "Not yet available" }}
                                        </span>
                                    </div>
                                }
                            }).collect_view()}

                            // "Request something else" row
                            <div
                                style=format!(
                                    "display:flex; align-items:center; padding:0.4rem 0.8rem; \
                                     cursor:pointer; color:{ACCENT}; font-size:0.8rem; \
                                     border-top:1px solid {BORDER};"
                                )
                                on:click=move |_| {
                                    show_custom.set(true);
                                    selected_entry.set(None);
                                    search_text.set(String::new());
                                    request_status.set(RequestStatus::Idle);
                                }
                            >
                                "📩 Request something else"
                            </div>
                        </div>
                    }.into_any()
                }}

                // ── Request panel (for unavailable curated opening) ─────────
                {move || {
                    match selected_entry.get() {
                        Some(SelectedEntry::Request(ref name)) => {
                            let opening_name = name.clone();
                            let opening_name_submit = name.clone();
                            view! {
                                <div style=format!(
                                    "border:1px solid {BORDER}; background:{PANEL}; \
                                     padding:0.75rem 1rem; font-size:0.82rem;"
                                )>
                                    <p style=format!("color:{TEXT}; margin:0 0 0.5rem 0;")>
                                        {format!("{} is not yet available.", opening_name)}
                                    </p>
                                    {move || request_panel_view(
                                        request_status,
                                        opening_name_submit.clone(),
                                        do_submit_request.clone(),
                                    ).into_any()}
                                </div>
                            }.into_any()
                        }
                        Some(SelectedEntry::Group(ref key)) => {
                            let desc = match key {
                                GroupKey::QueensGambit =>
                                    "★ Queen's Gambit Pack — randomly selects from all \
                                     Queen's Gambit, Slav, and Semi-Slav variations. \
                                     A different line is chosen each time you click Start.",
                                GroupKey::ModernDefense =>
                                    "★ Modern / Pirc Defense Pack — randomly selects from all \
                                     Modern Defense (...g6) and Pirc Defense (...d6) lines. \
                                     White plays realistic weighted-random responses.",
                                GroupKey::ItalianGame =>
                                    "★ Italian Game Pack — randomly selects from all \
                                     Italian Game variations (Giuoco Piano, Evans Gambit, \
                                     Two Knights). A different line is chosen each game.",
                                GroupKey::CaroKann =>
                                    "★ Caro-Kann Defense Pack — randomly selects from all \
                                     Caro-Kann Defense lines (Advance, Classical, Exchange). \
                                     White plays realistic weighted-random responses.",
                            };
                            view! {
                                <div style=format!(
                                    "border:1px solid {BORDER}; background:{PANEL}; \
                                     padding:0.5rem 0.8rem; font-size:0.78rem; \
                                     color:{ACCENT}; word-break:break-word; \
                                     font-style:italic;"
                                )>
                                    {desc}
                                </div>
                            }.into_any()
                        }
                        None => view! { <div style="display:none;" /> }.into_any()
                    }
                }}

                // ── Custom request panel (free-text "something else") ───────
                {move || {
                    if !show_custom.get() {
                        return view! { <div style="display:none;" /> }.into_any();
                    }

                    view! {
                        <div style=format!(
                            "border:1px solid {BORDER}; background:{PANEL}; \
                             padding:0.75rem 1rem; font-size:0.82rem;"
                        )>
                            <p style=format!("color:{TEXT}; margin:0 0 0.5rem 0;")>
                                "Describe the opening you'd like added:"
                            </p>
                            <input
                                type="text"
                                placeholder="e.g. Scandinavian Defense, King's Gambit…"
                                prop:value=move || custom_text.get()
                                style=format!("background:{BG}; color:{TEXT}; border:1px solid {BORDER}; \
                                               padding:0.4rem 0.8rem; font-family:'Courier New', monospace; \
                                               font-size:0.82rem; width:100%; box-sizing:border-box; \
                                               margin-bottom:0.5rem;")
                                on:input=move |e| custom_text.set(event_target_value(&e))
                            />
                            {move || {
                                let txt = custom_text.get();
                                request_panel_view(
                                    request_status,
                                    txt,
                                    do_submit_request.clone(),
                                ).into_any()
                            }}
                        </div>
                    }.into_any()
                }}
            </div>

            // ── Tip ─────────────────────────────────────────────────────────
            <p style=format!("color:{ACCENT}; font-size:0.72rem; opacity:0.6; \
                               max-width:480px; text-align:center; margin:0;")>
                "In opening mode the computer plays the opposing side automatically. \
                 Hint (1st click) shows the piece to move; (2nd click) shows the destination."
            </p>
        </div>
    }
}

// ── Request panel inner view ────────────────────────────────────────────────

fn request_panel_view(
    request_status: RwSignal<RequestStatus>,
    opening_name: String,
    do_submit: impl Fn(String) + Clone + 'static,
) -> impl IntoView {
    let status = request_status.get();
    let remaining = requests_remaining();
    let name_for_btn = opening_name.clone();
    let do_submit_clone = do_submit.clone();

    match status {
        RequestStatus::Success => {
            view! {
                <p style=format!("color:{ACCENT}; margin:0;")>
                    "✓ Request submitted! We'll review it soon."
                </p>
            }.into_any()
        }
        RequestStatus::Submitting => {
            view! {
                <p style=format!("color:{ACCENT}; margin:0;")>
                    "⏳ Submitting…"
                </p>
            }.into_any()
        }
        RequestStatus::RateLimited => {
            view! {
                <p style=format!("color:{MUTED}; margin:0;")>
                    "You've used both weekly requests. Try again in a few days."
                </p>
            }.into_any()
        }
        RequestStatus::QueueClosed => {
            view! {
                <p style=format!("color:{MUTED}; margin:0;")>
                    "Requests are temporarily closed — check back later."
                </p>
            }.into_any()
        }
        RequestStatus::Error(ref msg) => {
            let msg = msg.clone();
            view! {
                <p style=format!("color:#e63946; margin:0;")>
                    {format!("Error: {msg}")}
                </p>
            }.into_any()
        }
        RequestStatus::Idle => {
            if !REQUESTS_OPEN || FORMSPREE_ENDPOINT.is_none() {
                view! {
                    <p style=format!("color:{MUTED}; margin:0;")>
                        "Requests are temporarily closed — check back later."
                    </p>
                }.into_any()
            } else if remaining == 0 {
                view! {
                    <p style=format!("color:{MUTED}; margin:0;")>
                        "You've used both weekly requests. Try again in a few days."
                    </p>
                }.into_any()
            } else {
                view! {
                    <div>
                        <button
                            style=format!(
                                "background:{BTN_BG}; color:{TEXT}; border:1px solid {ACCENT}; \
                                 padding:0.4rem 1rem; font-family:'Courier New', monospace; \
                                 font-size:0.82rem; cursor:pointer; margin-bottom:0.3rem;"
                            )
                            disabled=move || opening_name.is_empty()
                            on:click=move |_| {
                                if !name_for_btn.is_empty() {
                                    do_submit_clone(name_for_btn.clone());
                                }
                            }
                        >
                            "📩 Request this opening"
                        </button>
                        <p style=format!("color:{MUTED}; font-size:0.75rem; margin:0;")>
                            {format!("{remaining} request(s) remaining this week")}
                        </p>
                    </div>
                }.into_any()
            }
        }
    }
}

// ── Style helpers ──────────────────────────────────────────────────────────

fn featured_card_style(selected: bool) -> String {
    let border = if selected { ACCENT }  else { BORDER };
    let bg     = if selected { SELECTED } else { PANEL };
    format!(
        "display:flex; flex-direction:column; align-items:center; \
         padding:0.75rem 1rem; border:2px solid {border}; background:{bg}; \
         cursor:pointer; min-width:200px; max-width:240px; text-align:center;"
    )
}

fn action_btn_style(enabled: bool) -> String {
    let opacity = if enabled { "1.0" } else { "0.4" };
    format!(
        "background:{BTN_BG}; color:{TEXT}; border:1px solid {ACCENT}; \
         padding:0.5rem 1.5rem; font-family:'Courier New', monospace; \
         font-size:0.9rem; letter-spacing:0.1em; cursor:pointer; \
         text-transform:uppercase; opacity:{opacity};"
    )
}
