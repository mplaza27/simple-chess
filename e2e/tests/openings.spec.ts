/**
 * openings.spec.ts — Tests for playing actual opening moves on the board.
 *
 * Verifies that:
 *  - Clicking the correct book move advances the opening counter
 *  - Wrong moves are blocked with progressive hints (1st=message, 2nd=piece, 3rd=full move)
 *  - The computer auto-plays its response within the timeout
 *  - Hints always point to the human's next move (both White and Black)
 *
 * Uses featured packs:
 *  - Queen's Gambit pack (as White): all variations start 1.d4
 *  - Modern / Pirc Defense pack (as Black): first move varies (g6 or d6) — use hint discovery
 */

import { test, expect } from '@playwright/test';
import { gotoSetup, waitForGameScreen, boardCell, findHintedSquares } from './helpers';

// ── Queen's Gambit Pack — playing as White ───────────────────────────────────
//
// All QG variations begin 1. d4 — d4 is always the correct first White move.
// Computer responds with d5, c6, e6, or Nf6 depending on the variation.

test.describe("Queen's Gambit pack — playing as White", () => {
  test.beforeEach(async ({ page }) => {
    await gotoSetup(page);
    await page.getByText("★ Queen's Gambit").click();
    await page.getByRole('button', { name: '▶ Start Opening' }).click();
    await waitForGameScreen(page);
  });

  test('starts at move 0/N with White to move', async ({ page }) => {
    await expect(page.getByText(/move 0\/\d+/)).toBeVisible();
    await expect(page.getByText(/Your turn \(White\)/)).toBeVisible();
  });

  test('playing d4 (the correct first book move) advances counter to move 1/N', async ({ page }) => {
    await boardCell(page, 'd2').click();
    await boardCell(page, 'd4').click();
    await expect(page.getByText(/move 1\/\d+/)).toBeVisible();
  });

  test('computer responds to d4 and counter reaches move 2/N within 8s', async ({ page }) => {
    await boardCell(page, 'd2').click();
    await boardCell(page, 'd4').click();

    await expect(page.getByText(/move 2\/\d+/)).toBeVisible({ timeout: 8_000 });
  });

  test('1st wrong move is blocked with "Not the book move" message', async ({ page }) => {
    await boardCell(page, 'e2').click();
    await boardCell(page, 'e4').click();
    await expect(page.getByText('Not the book move — try again')).toBeVisible();
    await expect(page.getByText(/move 0\/\d+/)).toBeVisible();
  });

  test('2nd wrong move shows piece hint', async ({ page }) => {
    // 1st wrong
    await boardCell(page, 'e2').click();
    await boardCell(page, 'e4').click();
    await expect(page.getByText('Not the book move — try again')).toBeVisible();
    // 2nd wrong
    await boardCell(page, 'e2').click();
    await boardCell(page, 'e4').click();
    await expect(page.getByText('Try the highlighted piece')).toBeVisible();
    await expect(page.getByRole('button', { name: '💡 Piece' })).toBeVisible();
    await expect(page.getByText(/move 0\/\d+/)).toBeVisible();
  });

  test('3rd wrong move shows full move hint', async ({ page }) => {
    // 1st wrong
    await boardCell(page, 'e2').click();
    await boardCell(page, 'e4').click();
    // 2nd wrong
    await boardCell(page, 'e2').click();
    await boardCell(page, 'e4').click();
    // 3rd wrong
    await boardCell(page, 'e2').click();
    await boardCell(page, 'e4').click();
    await expect(page.getByText('Play the highlighted move')).toBeVisible();
    await expect(page.getByRole('button', { name: '💡 Move' })).toBeVisible();
    await expect(page.getByText(/move 0\/\d+/)).toBeVisible();
  });

  test('correct move still works after wrong attempts', async ({ page }) => {
    // 1st wrong
    await boardCell(page, 'e2').click();
    await boardCell(page, 'e4').click();
    await expect(page.getByText('Not the book move — try again')).toBeVisible();
    // Now play correct move d4
    await boardCell(page, 'd2').click();
    await boardCell(page, 'd4').click();
    await expect(page.getByText(/move 1\/\d+/)).toBeVisible();
    // Hint should reset
    await expect(page.getByRole('button', { name: '💡 Hint' })).toBeVisible();
  });

  test('Undo after d4 restores move 0/N and re-dims Undo button', async ({ page }) => {
    await boardCell(page, 'd2').click();
    await boardCell(page, 'd4').click();
    await expect(page.getByText(/move 1\/\d+/)).toBeVisible();
    await expect(page.getByRole('button', { name: '↩ Undo' })).not.toHaveAttribute('style', /opacity:0\.4/);

    await page.getByRole('button', { name: '↩ Undo' }).click();
    await expect(page.getByText(/move 0\/\d+/)).toBeVisible();
    await expect(page.getByRole('button', { name: '↩ Undo' })).toHaveAttribute('style', /opacity:0\.4/);
  });

  test('playing d4 then c4 (both correct moves) advances to move 3/N', async ({ page }) => {
    await boardCell(page, 'd2').click();
    await boardCell(page, 'd4').click();
    await expect(page.getByText(/move 2\/\d+/)).toBeVisible({ timeout: 8_000 });

    await boardCell(page, 'c2').click();
    await boardCell(page, 'c4').click();
    await expect(page.getByText(/move 3\/\d+/)).toBeVisible();
  });
});

// ── Modern / Pirc Defense Pack — playing as Black ───────────────────────────
//
// MD/Pirc as Black: computer (White) plays first, then Black plays.
// First Black move varies by variation (g6 for Modern, d6 for Pirc),
// so we use findHintedSquares() to discover the correct move dynamically.
// Board is flipped (Black perspective) → use boardCell(page, sq, 'black').

test.describe('Modern / Pirc Defense pack — playing as Black', () => {
  test.beforeEach(async ({ page }) => {
    await gotoSetup(page);
    await page.getByText('★ Modern / Pirc Defense').click();
    await page.getByRole('button', { name: '▶ Start Opening' }).click();
    await waitForGameScreen(page);
    // Wait for computer (White) to play its first move
    await expect(page.getByText(/Your turn \(Black\)/)).toBeVisible({ timeout: 8_000 });
  });

  test('after computer moves, counter is at move 1/N with Black to move', async ({ page }) => {
    await expect(page.getByText(/move 1\/\d+/)).toBeVisible();
    await expect(page.getByText(/Your turn \(Black\)/)).toBeVisible();
  });

  test('hint shows Black\'s first book move', async ({ page }) => {
    await page.getByRole('button', { name: '💡 Hint' }).click();
    await page.getByRole('button', { name: '💡 Piece' }).click();

    const { from, to } = await findHintedSquares(page);
    expect(from).not.toBeNull();
    expect(to).not.toBeNull();
  });

  test('playing the correct first Black move advances counter to move 2/N', async ({ page }) => {
    // Use full hint to discover the correct move
    await page.getByRole('button', { name: '💡 Hint' }).click();
    await page.getByRole('button', { name: '💡 Piece' }).click();

    const { from, to } = await findHintedSquares(page);
    expect(from).not.toBeNull();
    expect(to).not.toBeNull();

    // Reset hint and play the discovered move
    await page.getByRole('button', { name: '💡 Move' }).click();
    await from!.click();
    await to!.click();
    await expect(page.getByText(/move 2\/\d+/)).toBeVisible();
  });

  test('computer responds to Black\'s move and counter reaches move 3/N within 8s', async ({ page }) => {
    // Discover and play the correct first Black move
    await page.getByRole('button', { name: '💡 Hint' }).click();
    await page.getByRole('button', { name: '💡 Piece' }).click();
    const { from, to } = await findHintedSquares(page);
    await page.getByRole('button', { name: '💡 Move' }).click();
    await from!.click();
    await to!.click();

    await expect(page.getByText(/move 3\/\d+/)).toBeVisible({ timeout: 8_000 });
  });

  test('wrong move is blocked with "Not the book move" message', async ({ page }) => {
    // Play e7→e5 — wrong for both Modern (g6) and Pirc (d6)
    await boardCell(page, 'e7', 'black').click();
    await boardCell(page, 'e5', 'black').click();
    await expect(page.getByText('Not the book move — try again')).toBeVisible();
    await expect(page.getByText(/move 1\/\d+/)).toBeVisible();
  });

  test('2nd wrong move as Black shows piece hint', async ({ page }) => {
    // 1st wrong
    await boardCell(page, 'e7', 'black').click();
    await boardCell(page, 'e5', 'black').click();
    // 2nd wrong
    await boardCell(page, 'e7', 'black').click();
    await boardCell(page, 'e5', 'black').click();
    await expect(page.getByText('Try the highlighted piece')).toBeVisible();
    await expect(page.getByRole('button', { name: '💡 Piece' })).toBeVisible();
  });

  test('Undo after playing correct move restores move 1/N', async ({ page }) => {
    // Discover and play the correct move
    await page.getByRole('button', { name: '💡 Hint' }).click();
    await page.getByRole('button', { name: '💡 Piece' }).click();
    const { from, to } = await findHintedSquares(page);
    await page.getByRole('button', { name: '💡 Move' }).click();
    await from!.click();
    await to!.click();
    await expect(page.getByText(/move 2\/\d+/)).toBeVisible();
    await expect(page.getByRole('button', { name: '↩ Undo' })).not.toHaveAttribute('style', /opacity:0\.4/);

    await page.getByRole('button', { name: '↩ Undo' }).click();
    // After undoing Black's move, we're back at move 1 (computer's move still applied)
    await expect(page.getByText(/move 1\/\d+/)).toBeVisible();
  });
});

// ── Italian Game Pack — playing as White ──────────────────────────────────
//
// All Italian Game variations begin 1. e4 e5 2. Nf3 Nc6 3. Bc4.
// First White move is always e2→e4.

test.describe('Italian Game pack — playing as White', () => {
  test.beforeEach(async ({ page }) => {
    await gotoSetup(page);
    await page.getByText('★ Italian Game').click();
    await page.getByRole('button', { name: '▶ Start Opening' }).click();
    await waitForGameScreen(page);
  });

  test('starts at move 0/N with White to move', async ({ page }) => {
    await expect(page.getByText(/move 0\/\d+/)).toBeVisible();
    await expect(page.getByText(/Your turn \(White\)/)).toBeVisible();
  });

  test('playing e4 (the correct first book move) advances counter to move 1/N', async ({ page }) => {
    await boardCell(page, 'e2').click();
    await boardCell(page, 'e4').click();
    await expect(page.getByText(/move 1\/\d+/)).toBeVisible();
  });

  test('computer responds to e4 and counter reaches move 2/N within 8s', async ({ page }) => {
    await boardCell(page, 'e2').click();
    await boardCell(page, 'e4').click();

    await expect(page.getByText(/move 2\/\d+/)).toBeVisible({ timeout: 8_000 });
  });

  test('1st wrong move is blocked with "Not the book move" message', async ({ page }) => {
    await boardCell(page, 'd2').click();
    await boardCell(page, 'd4').click();
    await expect(page.getByText('Not the book move — try again')).toBeVisible();
    await expect(page.getByText(/move 0\/\d+/)).toBeVisible();
  });

  test('correct move still works after wrong attempts', async ({ page }) => {
    // 1st wrong
    await boardCell(page, 'd2').click();
    await boardCell(page, 'd4').click();
    await expect(page.getByText('Not the book move — try again')).toBeVisible();
    // Now play correct move e4
    await boardCell(page, 'e2').click();
    await boardCell(page, 'e4').click();
    await expect(page.getByText(/move 1\/\d+/)).toBeVisible();
    // Hint should reset
    await expect(page.getByRole('button', { name: '💡 Hint' })).toBeVisible();
  });
});

// ── Caro-Kann Defense Pack — playing as Black ────────────────────────────
//
// Caro-Kann as Black: computer (White) plays 1.e4 first, then Black plays 1...c6.
// Board is flipped (Black perspective) → use boardCell(page, sq, 'black').

test.describe('Caro-Kann Defense pack — playing as Black', () => {
  test.beforeEach(async ({ page }) => {
    await gotoSetup(page);
    await page.getByText('★ Caro-Kann Defense').click();
    await page.getByRole('button', { name: '▶ Start Opening' }).click();
    await waitForGameScreen(page);
    // Wait for computer (White) to play its first move
    await expect(page.getByText(/Your turn \(Black\)/)).toBeVisible({ timeout: 8_000 });
  });

  test('after computer moves, counter is at move 1/N with Black to move', async ({ page }) => {
    await expect(page.getByText(/move 1\/\d+/)).toBeVisible();
    await expect(page.getByText(/Your turn \(Black\)/)).toBeVisible();
  });

  test('playing the correct first Black move (c6) advances counter to move 2/N', async ({ page }) => {
    await boardCell(page, 'c7', 'black').click();
    await boardCell(page, 'c6', 'black').click();
    await expect(page.getByText(/move 2\/\d+/)).toBeVisible();
  });

  test('computer responds to c6 and counter reaches move 3/N within 8s', async ({ page }) => {
    await boardCell(page, 'c7', 'black').click();
    await boardCell(page, 'c6', 'black').click();

    await expect(page.getByText(/move 3\/\d+/)).toBeVisible({ timeout: 8_000 });
  });

  test('wrong move is blocked with "Not the book move" message', async ({ page }) => {
    // Play e7→e5 — wrong for Caro-Kann (should be c6)
    await boardCell(page, 'e7', 'black').click();
    await boardCell(page, 'e5', 'black').click();
    await expect(page.getByText('Not the book move — try again')).toBeVisible();
    await expect(page.getByText(/move 1\/\d+/)).toBeVisible();
  });

  test('Undo after playing correct move restores move 1/N', async ({ page }) => {
    await boardCell(page, 'c7', 'black').click();
    await boardCell(page, 'c6', 'black').click();
    await expect(page.getByText(/move 2\/\d+/)).toBeVisible();
    await expect(page.getByRole('button', { name: '↩ Undo' })).not.toHaveAttribute('style', /opacity:0\.4/);

    await page.getByRole('button', { name: '↩ Undo' }).click();
    // After undoing Black's move, we're back at move 1 (computer's move still applied)
    await expect(page.getByText(/move 1\/\d+/)).toBeVisible();
  });
});
