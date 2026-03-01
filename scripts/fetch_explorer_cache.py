#!/usr/bin/env python3
"""
Pre-fetch Lichess Explorer data for Queen's Gambit and Modern Defense.

Downloads opening statistics for key positions (both sides) and caches them
in assets/explorer_cache.json so the app works well offline and loads faster.

FENs are pre-computed for the most common training positions:
  - Queen's Gambit (player=White, computer plays Black)
  - Modern Defense (player=Black, computer plays White)

Usage: python3 scripts/fetch_explorer_cache.py
Run from the project root. Takes ~3-5 minutes (288 API calls at 0.5s delay).
"""

import json
import os
import time
import urllib.request
import urllib.parse

# ── Config ─────────────────────────────────────────────────────────────────
RATINGS     = [1400, 1600, 1800, 2000, 2200, 2500]
DELAY       = 1.5   # seconds between API calls (Lichess asks ~1 req/s max)
MAX_RETRIES = 4     # retry on 429 with exponential backoff
BASE_URL    = "https://explorer.lichess.ovh/lichess"
SCRIPT_DIR  = os.path.dirname(os.path.abspath(__file__))
CACHE_PATH  = os.path.join(SCRIPT_DIR, "..", "assets", "explorer_cache.json")

# ── Key positions (FEN + label) ─────────────────────────────────────────────
# Queen's Gambit — player is White, computer plays Black.
# These are positions where it's Black's turn (computer's decision point).
QG_FENS = [
    # Early
    ("QG: after 1.d4",
     "rnbqkbnr/pppppppp/8/8/3P4/8/PPP1PPPP/RNBQKBNR b KQkq - 0 1"),
    ("QG: after 1.d4 d5 2.c4",
     "rnbqkbnr/ppp1pppp/8/3p4/2PP4/8/PP2PPPP/RNBQKBNR b KQkq - 0 2"),

    # QGD — 3.Nc3
    ("QGD: after 2...e6 3.Nc3",
     "rnbqkbnr/ppp2ppp/4p3/3p4/2PP4/2N5/PP2PPPP/R1BQKBNR b KQkq - 1 3"),
    # QGD — 3.Nf3
    ("QGD: after 2...e6 3.Nf3",
     "rnbqkbnr/ppp2ppp/4p3/3p4/2PP4/5N2/PP2PPPP/RNBQKB1R b KQkq - 1 3"),
    # QGD Orthodox — 4.Bg5
    ("QGD Orthodox: after 3.Nc3 Nf6 4.Bg5",
     "rnbqkb1r/ppp2ppp/4pn2/3p2B1/2PP4/2N5/PP2PPPP/R2QKBNR b KQkq - 3 4"),
    # QGD Exchange — 4.cxd5
    ("QGD Exchange: after 3.Nc3 Nf6 4.cxd5",
     "rnbqkb1r/ppp2ppp/4pn2/3P4/3P4/2N5/PP2PPPP/R1BQKBNR b KQkq - 0 4"),
    # QGD — 4.e3
    ("QGD: after 3.Nc3 Nf6 4.e3",
     "rnbqkb1r/ppp2ppp/4pn2/3p4/2PP4/2N1P3/PP3PPP/R1BQKBNR b KQkq - 0 4"),
    # QGD — 4.Nf3
    ("QGD: after 3.Nc3 Nf6 4.Nf3",
     "rnbqkb1r/ppp2ppp/4pn2/3p4/2PP4/2N2N2/PP2PPPP/R1BQKB1R b KQkq - 1 4"),
    # QGD Orthodox — 4.Bg5 Be7 5.e3
    ("QGD Orthodox: after Bg5 Be7 5.e3",
     "rnbqk2r/ppp1bppp/4pn2/3p2B1/2PP4/2N1P3/PP3PPP/R2QKBNR b KQkq - 0 5"),
    # QGD Orthodox — 4.Bg5 Be7 5.Nf3
    ("QGD Orthodox: after Bg5 Be7 5.Nf3",
     "rnbqk2r/ppp1bppp/4pn2/3p2B1/2PP4/2N2N2/PP2PPPP/R2QKB1R b KQkq - 2 5"),
    # QGD — Lasker variation Bg5 h6 5.Bh4
    ("QGD Lasker: after Bg5 h6 5.Bh4",
     "rnbqkb1r/ppp2pp1/4pn1p/3p4/2PP3B/2N5/PP2PPPP/R2QKBNR b KQkq - 1 5"),
    # QGD — Bg5 h6 5.Bxf6
    ("QGD: after Bg5 h6 5.Bxf6",
     "rnbqkb1r/ppp2pp1/4pB1p/3p4/2PP4/2N5/PP2PPPP/R2QKBNR b KQkq - 0 5"),
    # Tarrasch — 3.Nd2
    ("Tarrasch: after 2...e6 3.Nd2",
     "rnbqkbnr/ppp2ppp/4p3/3p4/2PP4/8/PP1NPPPP/R1BQKBNR b KQkq - 1 3"),
    # Tarrasch — deeper
    ("Tarrasch: after 3.Nd2 Nf6 4.Ngf3",
     "rnbqkb1r/ppp2ppp/4pn2/3p4/2PP4/5N2/PP1NPPPP/R1BQKB1R b KQkq - 1 4"),

    # QGA — 2...dxc4 lines
    ("QGA: after 2...dxc4 3.Nf3",
     "rnbqkbnr/ppp1pppp/8/8/2pP4/5N2/PP2PPPP/RNBQKB1R b KQkq - 1 3"),
    ("QGA: after 2...dxc4 3.e3",
     "rnbqkbnr/ppp1pppp/8/8/2pP4/4P3/PP3PPP/RNBQKBNR b KQkq - 0 3"),
    ("QGA: after 2...dxc4 3.e4",
     "rnbqkbnr/ppp1pppp/8/8/2pPP3/8/PP3PPP/RNBQKBNR b KQkq - 0 3"),

    # Slav — 2...c6 lines
    ("Slav: after 2...c6 3.Nc3",
     "rnbqkbnr/pp2pppp/2p5/3p4/2PP4/2N5/PP2PPPP/R1BQKBNR b KQkq - 1 3"),
    ("Slav: after 2...c6 3.Nf3",
     "rnbqkbnr/pp2pppp/2p5/3p4/2PP4/5N2/PP2PPPP/RNBQKB1R b KQkq - 1 3"),
    ("Slav: after 3.Nc3 Nf6 4.Nf3",
     "rnbqkb1r/pp2pppp/2p2n2/3p4/2PP4/2N2N2/PP2PPPP/R1BQKB1R b KQkq - 1 4"),
    ("Slav: after 3.Nc3 Nf6 4.e3",
     "rnbqkb1r/pp2pppp/2p2n2/3p4/2PP4/2N1P3/PP3PPP/R1BQKBNR b KQkq - 0 4"),

    # Semi-Slav
    ("Semi-Slav: after 4.Nf3 e6 5.e3",
     "rnbqkb1r/pp3ppp/2p1pn2/3p4/2PP4/2N1PN2/PP3PPP/R1BQKB1R b KQkq - 0 5"),
    ("Semi-Slav: after 4.Nf3 e6 5.Bg5",
     "rnbqkb1r/pp3ppp/2p1pn2/3p2B1/2PP4/2N2N2/PP2PPPP/R2QKB1R b KQkq - 1 5"),

    # Exchange variation (White plays cxd5)
    ("Exchange: after 2...e6 3.cxd5 exd5 4.Nc3",
     "rnbqkbnr/ppp2ppp/8/3p4/3P4/2N5/PP2PPPP/R1BQKBNR b KQkq - 0 4"),
    ("Exchange: after 2...e6 3.cxd5 exd5 4.Bf4",
     "rnbqkbnr/ppp2ppp/8/3p4/3P1B2/8/PP2PPPP/RN1QKBNR b KQkq - 0 4"),
]

# Modern Defense — player is Black, computer plays White.
# These are positions where it's White's turn (computer's decision point).
MODERN_FENS = [
    # Starting position — computer chooses White's first move
    ("Modern: starting position (White to move)",
     "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"),

    # vs 1.e4
    ("Modern vs 1.e4: after 1.e4 g6",
     "rnbqkbnr/pppppp1p/6p1/8/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2"),
    ("Modern vs 1.e4: after 1.e4 g6 2.d4 Bg7",
     "rnbqk1nr/ppppppbp/6p1/8/3PP3/8/PPP2PPP/RNBQKBNR w KQkq - 2 3"),
    ("Modern vs 1.e4: after 3.Nc3 d6",
     "rnbqk1nr/ppp1ppbp/3p2p1/8/3PP3/2N5/PPP2PPP/R1BQKBNR w KQkq - 0 4"),
    ("Modern vs 1.e4: after 3.Nf3 d6",
     "rnbqk1nr/ppp1ppbp/3p2p1/8/3PP3/5N2/PPP2PPP/RNBQKB1R w KQkq - 0 4"),
    ("Modern vs 1.e4: after 3.Bc4 d6",
     "rnbqk1nr/ppp1ppbp/3p2p1/8/2BPP3/8/PPP2PPP/RNBQK1NR w KQkq - 0 4"),
    ("Modern vs 1.e4: after 3.f4 d6",
     "rnbqk1nr/ppp1ppbp/3p2p1/8/3PPP2/8/PPP3PP/RNBQKBNR w KQkq - 0 4"),
    ("Modern vs 1.e4: after 3.Nc3 d6 4.f4 Nf6",
     "rnbqk2r/ppp1ppbp/3p1np1/8/3PPP2/2N5/PPP3PP/R1BQKBNR w KQkq - 1 5"),
    ("Modern vs 1.e4: after 3.Nc3 d6 4.Nf3 Nf6",
     "rnbqk2r/ppp1ppbp/3p1np1/8/3PP3/2N2N2/PPP2PPP/R1BQKB1R w KQkq - 1 5"),
    ("Modern vs 1.e4: after 3.Nc3 d6 4.Be3 Nf6",
     "rnbqk2r/ppp1ppbp/3p1np1/8/3PP3/2N1B3/PPP2PPP/R2QKBNR w KQkq - 1 5"),

    # vs 1.d4
    ("Modern vs 1.d4: after 1.d4 g6",
     "rnbqkbnr/pppppp1p/6p1/8/3P4/8/PPP1PPPP/RNBQKBNR w KQkq - 0 2"),
    ("Modern vs 1.d4: after 2.c4 Bg7",
     "rnbqk1nr/ppppppbp/6p1/8/2PP4/8/PP2PPPP/RNBQKBNR w KQkq - 2 3"),
    ("Modern vs 1.d4: after 3.Nc3 d6",
     "rnbqk1nr/ppp1ppbp/3p2p1/8/2PP4/2N5/PP2PPPP/R1BQKBNR w KQkq - 0 4"),
    ("Modern vs 1.d4: after 3.e4 d6",
     "rnbqk1nr/ppp1ppbp/3p2p1/8/2PPP3/8/PP3PPP/RNBQKBNR w KQkq - 0 4"),
    ("Modern vs 1.d4: after 3.Nc3 d6 4.e4 Nc6",
     "r1bqk1nr/ppp1ppbp/2np2p1/8/2PPP3/2N5/PP3PPP/R1BQKBNR w KQkq - 1 5"),
    ("Modern vs 1.d4: after 3.Nc3 d6 4.e4 f5",
     "rnbqk1nr/ppp1ppbp/3p2p1/5p2/2PPP3/2N5/PP3PPP/R1BQKBNR w KQkq f6 0 5"),

    # vs 1.c4
    ("Modern vs 1.c4: after 1.c4 g6",
     "rnbqkbnr/pppppp1p/6p1/8/2P5/8/PP1PPPPP/RNBQKBNR w KQkq - 0 2"),
    ("Modern vs 1.c4: after 2.e4 Bg7",
     "rnbqk1nr/ppppppbp/6p1/8/2P1P3/8/PP1P1PPP/RNBQKBNR w KQkq - 2 3"),
    ("Modern vs 1.c4: after 2.Nc3 Bg7",
     "rnbqk1nr/ppppppbp/6p1/8/2P5/2N5/PP1PPPPP/R1BQKBNR w KQkq - 2 3"),

    # vs 1.Nf3
    ("Modern vs 1.Nf3: after 1.Nf3 g6",
     "rnbqkbnr/pppppp1p/6p1/8/8/5N2/PPPPPPPP/RNBQKB1R w KQkq - 0 2"),
    ("Modern vs 1.Nf3: after 2.d4 Bg7",
     "rnbqk1nr/ppppppbp/6p1/8/3P4/5N2/PPP1PPPP/RNBQKB1R w KQkq - 2 3"),
    ("Modern vs 1.Nf3: after 2.c4 Bg7",
     "rnbqk1nr/ppppppbp/6p1/8/2P5/5N2/PP1PPPPP/RNBQKB1R w KQkq - 2 3"),
    ("Modern vs 1.Nf3: after 2.d4 Bg7 3.e4 d6",
     "rnbqk1nr/ppp1ppbp/3p2p1/8/3PP3/5N2/PPP2PPP/RNBQKB1R w KQkq - 0 4"),
]

# ── API fetch ───────────────────────────────────────────────────────────────

def fetch_explorer(fen: str, rating: int) -> dict | None:
    url = (
        f"{BASE_URL}"
        f"?fen={urllib.parse.quote(fen)}"
        f"&ratings={rating}"
        f"&speeds=rapid,classical"
    )
    req = urllib.request.Request(
        url,
        headers={"User-Agent": "chess-trainer-cache-builder/1.0"},
    )
    backoff = 5.0
    for attempt in range(MAX_RETRIES):
        try:
            with urllib.request.urlopen(req, timeout=20) as resp:
                data = json.loads(resp.read().decode())
                if data.get("moves"):
                    return data
                return None  # valid response but no moves (terminal position)
        except urllib.error.HTTPError as e:
            if e.code == 429:
                wait = backoff * (2 ** attempt)
                print(f"    429 rate-limited — waiting {wait:.0f}s...", end="", flush=True)
                time.sleep(wait)
                print(" retrying")
            else:
                print(f"    ERROR: HTTP {e.code}")
                return None
        except Exception as e:
            print(f"    ERROR: {e}")
            return None
    print(f"    GAVE UP after {MAX_RETRIES} retries")
    return None

# ── Main ────────────────────────────────────────────────────────────────────

def main():
    # Load existing cache
    try:
        with open(CACHE_PATH) as f:
            cache: dict = json.load(f)
        print(f"Loaded {len(cache)} existing cache entries from {CACHE_PATH}")
    except (FileNotFoundError, json.JSONDecodeError):
        cache = {}
        print("Starting with empty cache")

    all_positions = [
        *[(label, fen) for label, fen in QG_FENS],
        *[(label, fen) for label, fen in MODERN_FENS],
    ]

    total_calls  = len(all_positions) * len(RATINGS)
    fetched      = 0
    skipped      = 0
    errors       = 0

    print(f"\n{len(all_positions)} positions × {len(RATINGS)} ratings = {total_calls} API calls")
    print(f"Estimated time: {total_calls * DELAY / 60:.1f} minutes\n")

    try:
        for i, (label, fen) in enumerate(all_positions):
            print(f"[{i+1}/{len(all_positions)}] {label}")
            for rating in RATINGS:
                key = f"{fen}|{rating}"
                if key in cache:
                    skipped += 1
                    print(f"  {rating}: cached ({len(cache[key]['moves'])} moves)")
                    continue

                print(f"  {rating}: fetching... ", end="", flush=True)
                data = fetch_explorer(fen, rating)
                if data:
                    cache[key] = data
                    fetched += 1
                    print(f"{len(data['moves'])} moves")
                else:
                    errors += 1
                    print("no data")
                time.sleep(DELAY)

    except KeyboardInterrupt:
        print("\nInterrupted — saving partial cache...")

    # Save
    with open(CACHE_PATH, "w") as f:
        json.dump(cache, f)

    print(f"\nDone!")
    print(f"  Fetched: {fetched}")
    print(f"  Skipped: {skipped} (already cached)")
    print(f"  Errors:  {errors}")
    print(f"  Total entries in cache: {len(cache)}")
    print(f"  Saved to: {CACHE_PATH}")


if __name__ == "__main__":
    main()
