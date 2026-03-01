import { Page, Locator } from '@playwright/test';

/**
 * Navigate to the app and wait for both phases of WASM+data loading:
 *  1. WASM initializes  → heading "♟ Simple Chess ♟" becomes visible
 *  2. 5 TSV fetches complete (Suspense resolves) → hidden data-loaded marker appears
 */
export async function gotoSetup(page: Page): Promise<void> {
  await page.goto('/');
  // Phase 1: WASM init
  await page.getByRole('heading', { name: '♟ Simple Chess ♟' }).waitFor({ state: 'visible' });
  // Phase 2: opening data loaded (Suspense resolved — hidden span with data-loaded="true")
  await page.locator('[data-loaded="true"]').waitFor({ state: 'attached' });
}

/**
 * Wait for the game screen to be visible (← Setup button is the canary).
 */
export async function waitForGameScreen(page: Page): Promise<void> {
  await page.getByRole('button', { name: '← Setup' }).waitFor({ state: 'visible' });
}

/**
 * Returns a Playwright locator for a specific board square by algebraic notation.
 *
 * The board grid renders squares in row-major order based on perspective:
 *
 * **White perspective** (rank 8 at top, file a at left):
 *   a8 (cell 0) → h8 (cell 7) → a7 (cell 8) → … → h1 (cell 63)
 *   idx = (7 - rank_0indexed) * 8 + file_0indexed
 *
 * **Black perspective** (rank 1 at top, file h at left):
 *   h1 (cell 0) → a1 (cell 7) → h2 (cell 8) → … → a8 (cell 63)
 *   idx = rank_0indexed * 8 + (7 - file_0indexed)
 *
 * @param page  - Playwright Page object
 * @param sq    - Algebraic square notation, e.g. "e4", "d2", "g7"
 * @param color - Board perspective: 'white' (default) or 'black'
 */
export function boardCell(page: Page, sq: string, color: 'white' | 'black' = 'white'): Locator {
  const file = sq.charCodeAt(0) - 97; // 'a'=97; a→0 … h→7
  const rank = parseInt(sq[1]) - 1;   // '1'→0 … '8'→7
  const idx = color === 'white'
    ? (7 - rank) * 8 + file
    : rank * 8 + (7 - file);
  // The 8×8 grid has style "grid-template-columns:repeat(8,..." (distinct from
  // the file-label row which starts with "1.4rem repeat(8,...").
  return page
    .locator('div[style*="grid-template-columns:repeat(8,"]')
    .locator('> div')
    .nth(idx);
}

// Board square color constants (must match board.rs)
const HINT_FROM = '#7b61ff'; // dark violet — piece to move
const HINT_TO   = '#a599ff'; // light violet — destination square

/**
 * Scans the 64 board cells to find which squares have hint highlighting.
 * Returns the `from` and `to` locators (or null if not found).
 *
 * Useful for MD/Pirc tests where the first Black move varies by variation
 * (g7→g6 for Modern, d7→d6 for Pirc).
 */
export async function findHintedSquares(page: Page): Promise<{
  from: Locator | null;
  to: Locator | null;
}> {
  const grid = page.locator('div[style*="grid-template-columns:repeat(8,"]');
  const cells = grid.locator('> div');
  let from: Locator | null = null;
  let to: Locator | null = null;

  const count = await cells.count();
  for (let i = 0; i < count; i++) {
    const cell = cells.nth(i);
    const style = await cell.getAttribute('style') ?? '';
    if (style.includes(HINT_FROM)) {
      from = cell;
    } else if (style.includes(HINT_TO)) {
      to = cell;
    }
  }
  return { from, to };
}
