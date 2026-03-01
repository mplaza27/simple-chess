import { test, expect } from '@playwright/test';
import { gotoSetup, waitForGameScreen, boardCell, findHintedSquares } from './helpers';

// Board square color constants (must match board.rs)
const HINT_FROM = '#7b61ff'; // dark violet — piece to move
const HINT_TO   = '#a599ff'; // light violet — destination square

// ── Helpers ───────────────────────────────────────────────────────────────────

/** Start QG pack as White — all variations begin 1.d4. */
async function startQGAsWhite(page: Parameters<typeof gotoSetup>[0]) {
  await gotoSetup(page);
  await page.getByText("★ Queen's Gambit").click();
  await page.getByRole('button', { name: '▶ Start Opening' }).click();
  await waitForGameScreen(page);
}

/** Start MD/Pirc pack as Black — color is inferred from pack. */
async function startMDAsBlack(page: Parameters<typeof gotoSetup>[0]) {
  await gotoSetup(page);
  await page.getByText('★ Modern / Pirc Defense').click();
  await page.getByRole('button', { name: '▶ Start Opening' }).click();
  await waitForGameScreen(page);
}

/** Start Italian Game pack as White — all variations begin 1.e4 e5 2.Nf3 Nc6 3.Bc4. */
async function startItalianAsWhite(page: Parameters<typeof gotoSetup>[0]) {
  await gotoSetup(page);
  await page.getByText('★ Italian Game').click();
  await page.getByRole('button', { name: '▶ Start Opening' }).click();
  await waitForGameScreen(page);
}

/** Start Caro-Kann Defense pack as Black — all variations begin 1.e4 c6. */
async function startCKAsBlack(page: Parameters<typeof gotoSetup>[0]) {
  await gotoSetup(page);
  await page.getByText('★ Caro-Kann Defense').click();
  await page.getByRole('button', { name: '▶ Start Opening' }).click();
  await waitForGameScreen(page);
}

// ── Board presence ────────────────────────────────────────────────────────────

test.describe('Board presence', () => {
  test('board grid has exactly 64 cells', async ({ page }) => {
    await startQGAsWhite(page);
    const boardGrid = page.locator('div[style*="grid-template-columns:repeat(8,"]');
    const cells = boardGrid.locator('> div');
    await expect(cells).toHaveCount(64);
  });

  test('rank labels 1–8 are visible', async ({ page }) => {
    await startQGAsWhite(page);
    for (let rank = 1; rank <= 8; rank++) {
      await expect(page.getByText(String(rank)).first()).toBeVisible();
    }
  });

  test('file labels a–h are visible', async ({ page }) => {
    await startQGAsWhite(page);
    for (const file of ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h']) {
      await expect(page.getByText(file).first()).toBeVisible();
    }
  });

  test('status shows "Your turn (White)" at start', async ({ page }) => {
    await startQGAsWhite(page);
    await expect(page.getByText(/Your turn \(White\)/)).toBeVisible();
  });
});

// ── Control buttons ───────────────────────────────────────────────────────────

test.describe('Control buttons', () => {
  test('New Game, ↩ Undo, and ← Setup are all present', async ({ page }) => {
    await startQGAsWhite(page);
    await expect(page.getByRole('button', { name: 'New Game' })).toBeVisible();
    await expect(page.getByRole('button', { name: '↩ Undo' })).toBeVisible();
    await expect(page.getByRole('button', { name: '← Setup' })).toBeVisible();
  });

  test('↩ Undo is visually dimmed (opacity:0.4) at game start', async ({ page }) => {
    await startQGAsWhite(page);
    const undo = page.getByRole('button', { name: '↩ Undo' });
    await expect(undo).toHaveAttribute('style', /opacity:0\.4/);
  });

  test('hint button "💡 Hint" is present in opening mode (White\'s turn)', async ({ page }) => {
    await startQGAsWhite(page);
    await expect(page.getByRole('button', { name: '💡 Hint' })).toBeVisible();
  });
});

// ── Hint cycle ────────────────────────────────────────────────────────────────

test.describe('Hint cycle', () => {
  test('hint button cycles Off → Piece → Move → Off', async ({ page }) => {
    await startQGAsWhite(page);

    const hintBtn = page.getByRole('button', { name: /💡/ });
    await expect(hintBtn).toHaveText('💡 Hint');

    await hintBtn.click();
    await expect(page.getByRole('button', { name: '💡 Piece' })).toBeVisible();

    await page.getByRole('button', { name: '💡 Piece' }).click();
    await expect(page.getByRole('button', { name: '💡 Move' })).toBeVisible();

    await page.getByRole('button', { name: '💡 Move' }).click();
    await expect(page.getByRole('button', { name: '💡 Hint' })).toBeVisible();
  });
});

// ── Hint highlights — White side (QG pack) ──────────────────────────────────
//
// QG: all variations start 1.d4. White's first book move: d2 → d4.
// Board is White perspective → boardCell(page, sq, 'white').

test.describe('Hint highlights — White (QG)', () => {
  test.beforeEach(async ({ page }) => {
    await startQGAsWhite(page);
  });

  test('💡 Piece: from-square (d2) turns violet, to-square (d4) stays normal', async ({ page }) => {
    await page.getByRole('button', { name: '💡 Hint' }).click();
    await expect(page.getByRole('button', { name: '💡 Piece' })).toBeVisible();

    await expect(boardCell(page, 'd2', 'white')).toHaveAttribute('style', new RegExp(`background:${HINT_FROM.replace('#', '\\#')}`));
    await expect(boardCell(page, 'd4', 'white')).not.toHaveAttribute('style', new RegExp(`background:${HINT_TO.replace('#', '\\#')}`));
  });

  test('💡 Move: both from-square (d2) and to-square (d4) are violet', async ({ page }) => {
    await page.getByRole('button', { name: '💡 Hint' }).click();
    await page.getByRole('button', { name: '💡 Piece' }).click();
    await expect(page.getByRole('button', { name: '💡 Move' })).toBeVisible();

    await expect(boardCell(page, 'd2', 'white')).toHaveAttribute('style', new RegExp(`background:${HINT_FROM.replace('#', '\\#')}`));
    await expect(boardCell(page, 'd4', 'white')).toHaveAttribute('style', new RegExp(`background:${HINT_TO.replace('#', '\\#')}`));
  });

  test('hint resets after playing the correct move (d2→d4)', async ({ page }) => {
    await page.getByRole('button', { name: '💡 Hint' }).click();
    await expect(boardCell(page, 'd2', 'white')).toHaveAttribute('style', new RegExp(`background:${HINT_FROM.replace('#', '\\#')}`));

    await boardCell(page, 'd2', 'white').click();
    await boardCell(page, 'd4', 'white').click();

    await expect(page.getByRole('button', { name: '💡 Hint' })).toBeVisible();
    await expect(boardCell(page, 'd2', 'white')).not.toHaveAttribute('style', new RegExp(`background:${HINT_FROM.replace('#', '\\#')}`));
  });
});

// ── Hint highlights — Black side (MD/Pirc pack) ─────────────────────────────
//
// MD/Pirc as Black: computer (White) plays first, then hint shows Black's
// book move. The first Black move varies by variation (g7→g6 or d7→d6),
// so we use findHintedSquares() to discover it dynamically.

test.describe('Hint highlights — Black (MD/Pirc)', () => {
  test.beforeEach(async ({ page }) => {
    await startMDAsBlack(page);
    // Wait for computer (White) to play its first move
    await expect(page.getByText(/Your turn \(Black\)/)).toBeVisible({ timeout: 8_000 });
  });

  test('💡 Piece: from-square turns violet on Black\'s board', async ({ page }) => {
    await page.getByRole('button', { name: '💡 Hint' }).click();
    await expect(page.getByRole('button', { name: '💡 Piece' })).toBeVisible();

    const { from } = await findHintedSquares(page);
    expect(from).not.toBeNull();
    await expect(from!).toHaveAttribute('style', new RegExp(`background:${HINT_FROM.replace('#', '\\#')}`));
  });

  test('💡 Move: both from and to squares are violet on Black\'s board', async ({ page }) => {
    await page.getByRole('button', { name: '💡 Hint' }).click();
    await page.getByRole('button', { name: '💡 Piece' }).click();
    await expect(page.getByRole('button', { name: '💡 Move' })).toBeVisible();

    const { from, to } = await findHintedSquares(page);
    expect(from).not.toBeNull();
    expect(to).not.toBeNull();
    await expect(from!).toHaveAttribute('style', new RegExp(`background:${HINT_FROM.replace('#', '\\#')}`));
    await expect(to!).toHaveAttribute('style', new RegExp(`background:${HINT_TO.replace('#', '\\#')}`));
  });

  test('hint resets after playing the correct move', async ({ page }) => {
    // Activate full hint to discover the correct move
    await page.getByRole('button', { name: '💡 Hint' }).click();
    await page.getByRole('button', { name: '💡 Piece' }).click();
    await expect(page.getByRole('button', { name: '💡 Move' })).toBeVisible();

    const { from, to } = await findHintedSquares(page);
    expect(from).not.toBeNull();
    expect(to).not.toBeNull();

    // Play the hinted move
    await from!.click();
    await to!.click();

    await expect(page.getByRole('button', { name: '💡 Hint' })).toBeVisible();
  });
});

// ── Auto-play as Black ────────────────────────────────────────────────────────

test.describe('Auto-play as Black (MD/Pirc)', () => {
  test('computer (White) plays first when human is Black', async ({ page }) => {
    await startMDAsBlack(page);
    await expect(page.getByText('Computer thinking (White)')).toBeVisible();
  });

  test('computer move completes within 8s and shows "Your turn (Black)"', async ({ page }) => {
    await startMDAsBlack(page);
    await expect(page.getByText(/Your turn \(Black\)/)).toBeVisible({ timeout: 8_000 });
    await expect(page.getByText(/move 1\/\d+/)).toBeVisible();
  });
});

// ── New Game ──────────────────────────────────────────────────────────────────

test.describe('New Game', () => {
  test('New Game resets status and move counter to 0/N', async ({ page }) => {
    await startQGAsWhite(page);

    await page.getByRole('button', { name: 'New Game' }).click();

    await expect(page.getByText(/Your turn \(White\)/)).toBeVisible();
    await expect(page.getByText(/move 0\/\d+/)).toBeVisible();
    await expect(page.getByRole('button', { name: '↩ Undo' })).toHaveAttribute('style', /opacity:0\.4/);
  });
});
