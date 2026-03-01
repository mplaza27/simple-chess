//! # explorer.rs — Lichess Explorer API Client + Cache
//!
//! Provides weighted-random opponent move selection based on actual game
//! statistics from the Lichess opening explorer.
//!
//! ## Three-tier lookup
//! 1. **localStorage cache** (WASM only) — instant, avoids redundant API calls
//! 2. **Bundled cache** — a JSON file compiled in via `include_str!`
//! 3. **Live API** (WASM only) — fetches from `explorer.lichess.ovh`
//!
//! ## Offline resilience
//! If the API is unreachable, the caller falls back to the deterministic book
//! move. The app never panics on network failure.

use serde::{Deserialize, Serialize};
use shakmaty::Square;

#[cfg(target_arch = "wasm32")]
use gloo_net::http::Request;

#[cfg(target_arch = "wasm32")]
use gloo_storage::{LocalStorage, Storage};

// ── Data types ─────────────────────────────────────────────────────────────

/// A single candidate move from the Explorer API, with game counts.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExplorerMove {
    pub uci: String,
    pub san: String,
    pub white: u64,
    pub draws: u64,
    pub black: u64,
}

impl ExplorerMove {
    /// Total number of games in which this move was played.
    pub fn total(&self) -> u64 {
        self.white + self.draws + self.black
    }
}

/// Partial Explorer API response — we only need the `moves` array.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExplorerResponse {
    pub moves: Vec<ExplorerMove>,
}

// ── Rating band mapping ───────────────────────────────────────────────────

/// Maps a slider rating (800–2500) to the nearest Explorer API band.
///
/// The Lichess Explorer API accepts specific rating brackets:
/// 0, 1000, 1200, 1400, 1600, 1800, 2000, 2200, 2500.
/// We map to the subset that gives meaningful differentiation.
pub fn rating_to_band(rating: u16) -> u16 {
    match rating {
        0..=1399    => 1400,
        1400..=1599 => 1600,
        1600..=1799 => 1800,
        1800..=2099 => 2000,
        2100..=2349 => 2200,
        _           => 2500,
    }
}

// ── Cache key ─────────────────────────────────────────────────────────────

fn cache_key(fen: &str, band: u16) -> String {
    format!("{fen}|{band}")
}

// ── Bundled cache ─────────────────────────────────────────────────────────

/// The bundled explorer cache, compiled in from `assets/explorer_cache.json`.
/// Starts empty (`{}`); can be pre-populated with a generation script later.
static BUNDLED_CACHE: &str = include_str!("../assets/explorer_cache.json");

fn lookup_bundled(key: &str) -> Option<ExplorerResponse> {
    let map: serde_json::Value = serde_json::from_str(BUNDLED_CACHE).ok()?;
    let entry = map.get(key)?;
    serde_json::from_value(entry.clone()).ok()
}

// ── localStorage cache (WASM only) ────────────────────────────────────────

#[cfg(target_arch = "wasm32")]
fn lookup_local_storage(key: &str) -> Option<ExplorerResponse> {
    let json: String = LocalStorage::get(key).ok()?;
    serde_json::from_str(&json).ok()
}

#[cfg(target_arch = "wasm32")]
fn save_local_storage(key: &str, resp: &ExplorerResponse) {
    if let Ok(json) = serde_json::to_string(resp) {
        let _ = LocalStorage::set(key, json);
    }
}

// ── Live API fetch (WASM only) ────────────────────────────────────────────

#[cfg(target_arch = "wasm32")]
async fn fetch_explorer_api(fen: &str, band: u16) -> Option<ExplorerResponse> {
    let url = format!(
        "https://explorer.lichess.ovh/lichess?fen={}&ratings={}&speeds=rapid,classical",
        urlencoding(fen),
        band
    );
    let resp = Request::get(&url).send().await.ok()?;
    let text = resp.text().await.ok()?;
    serde_json::from_str(&text).ok()
}

/// Minimal percent-encoding for FEN strings in URLs.
/// FEN contains spaces and slashes that need encoding.
#[cfg(target_arch = "wasm32")]
fn urlencoding(s: &str) -> String {
    s.replace(' ', "%20")
        .replace('/', "%2F")
}

// ── Public lookup function ────────────────────────────────────────────────

/// Look up Explorer data for a position at a given rating.
///
/// Three-tier lookup: localStorage → bundled cache → live API (WASM only).
/// On native (tests), only the bundled cache is checked.
///
/// Returns `None` if no data is available (caller falls back to book move).
pub async fn lookup_explorer(fen: &str, rating: u16) -> Option<ExplorerResponse> {
    let band = rating_to_band(rating);
    let key = cache_key(fen, band);

    // Tier 1: localStorage (WASM only)
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(cached) = lookup_local_storage(&key) {
            return Some(cached);
        }
    }

    // Tier 2: bundled cache
    if let Some(bundled) = lookup_bundled(&key) {
        return Some(bundled);
    }

    // Tier 3: live API (WASM only)
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(resp) = fetch_explorer_api(fen, band).await {
            // Cache for next time
            save_local_storage(&key, &resp);
            return Some(resp);
        }
    }

    None
}

// ── Weighted random move selection ────────────────────────────────────────

/// Pick a move from Explorer data using weighted random selection.
///
/// Each move is weighted by its total game count. At lower ratings (below
/// 1600), there's an additional chance of picking a non-top move to simulate
/// less perfect play: P(non-top) = (1600 - rating) / 800.
///
/// On native (tests), always picks the most popular move deterministically.
pub fn pick_explorer_move(resp: &ExplorerResponse, _rating: u16) -> Option<ExplorerMove> {
    if resp.moves.is_empty() {
        return None;
    }

    #[cfg(target_arch = "wasm32")]
    {
        let rating = _rating;
        let mut moves = resp.moves.clone();

        // Sort by popularity (descending) so index 0 = most popular
        moves.sort_by(|a, b| b.total().cmp(&a.total()));

        // At sub-1600 ratings, sometimes avoid the top move
        if rating < 1600 && moves.len() > 1 {
            let p_nontop = (1600.0 - rating as f64) / 800.0;
            let roll = js_sys::Math::random();
            if roll < p_nontop {
                // Pick from non-top moves weighted by game count
                let non_top: Vec<ExplorerMove> = moves[1..].to_vec();
                return Some(weighted_pick_wasm(&non_top));
            }
        }

        // Normal weighted selection from all moves
        return Some(weighted_pick_wasm(&moves));
    }

    // Native: deterministic — pick the most popular move
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut moves = resp.moves.clone();
        moves.sort_by(|a, b| b.total().cmp(&a.total()));
        Some(moves[0].clone())
    }
}

/// Weighted random pick using `js_sys::Math::random()`.
#[cfg(target_arch = "wasm32")]
fn weighted_pick_wasm(moves: &[ExplorerMove]) -> ExplorerMove {
    let total_weight: u64 = moves.iter().map(|m| m.total()).sum();
    if total_weight == 0 {
        return moves[0].clone();
    }

    let roll = (js_sys::Math::random() * total_weight as f64) as u64;
    let mut cumulative = 0u64;
    for m in moves {
        cumulative += m.total();
        if roll < cumulative {
            return m.clone();
        }
    }
    // Fallback (shouldn't happen due to rounding)
    moves.last().unwrap().clone()
}

// ── UCI square parser ─────────────────────────────────────────────────────

/// Parse a UCI move string like "e2e4" into (from, to) squares.
///
/// Also handles promotion moves like "e7e8q" by ignoring the suffix.
pub fn parse_uci_squares(uci: &str) -> Option<(Square, Square)> {
    if uci.len() < 4 {
        return None;
    }
    let from = parse_square(&uci[0..2])?;
    let to = parse_square(&uci[2..4])?;
    Some((from, to))
}

fn parse_square(s: &str) -> Option<Square> {
    let bytes = s.as_bytes();
    if bytes.len() < 2 { return None; }
    let file = bytes[0].checked_sub(b'a')?;
    let rank = bytes[1].checked_sub(b'1')?;
    if file > 7 || rank > 7 { return None; }
    Some(Square::from_coords(
        shakmaty::File::new(file.into()),
        shakmaty::Rank::new(rank.into()),
    ))
}

// ── Unit tests ────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rating_band_mapping() {
        assert_eq!(rating_to_band(800), 1400);
        assert_eq!(rating_to_band(1200), 1400);
        assert_eq!(rating_to_band(1600), 1800);
        assert_eq!(rating_to_band(1800), 2000);
        assert_eq!(rating_to_band(2000), 2000);
        assert_eq!(rating_to_band(2200), 2200);
        assert_eq!(rating_to_band(2500), 2500);
    }

    #[test]
    fn parse_uci_e2e4() {
        let (from, to) = parse_uci_squares("e2e4").unwrap();
        assert_eq!(from, Square::E2);
        assert_eq!(to, Square::E4);
    }

    #[test]
    fn parse_uci_promotion() {
        let (from, to) = parse_uci_squares("e7e8q").unwrap();
        assert_eq!(from, Square::E7);
        assert_eq!(to, Square::E8);
    }

    #[test]
    fn parse_uci_invalid() {
        assert!(parse_uci_squares("z9").is_none());
        assert!(parse_uci_squares("").is_none());
    }

    #[test]
    fn pick_deterministic_on_native() {
        let resp = ExplorerResponse {
            moves: vec![
                ExplorerMove {
                    uci: "e2e4".into(), san: "e4".into(),
                    white: 100, draws: 50, black: 50,
                },
                ExplorerMove {
                    uci: "d2d4".into(), san: "d4".into(),
                    white: 200, draws: 100, black: 100,
                },
            ],
        };
        // On native, should always pick the most popular (d2d4 with total 400)
        let picked = pick_explorer_move(&resp, 1600).unwrap();
        assert_eq!(picked.uci, "d2d4");
    }

    #[test]
    fn empty_response_returns_none() {
        let resp = ExplorerResponse { moves: vec![] };
        assert!(pick_explorer_move(&resp, 1600).is_none());
    }

    #[test]
    fn bundled_cache_returns_none_for_empty() {
        // The bundled cache starts as `{}`, so any lookup should return None
        let result = lookup_bundled("some_fen|1800");
        assert!(result.is_none());
    }
}
