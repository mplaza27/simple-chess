import { test, expect } from '@playwright/test';
import { gotoSetup } from './helpers';

// ── Initial state ────────────────────────────────────────────────────────────

test.describe('Initial state', () => {
  test('all key elements are visible on load', async ({ page }) => {
    await gotoSetup(page);

    await expect(page.getByRole('heading', { name: '♟ Simple Chess ♟' })).toBeVisible();
    await expect(page.getByText('Opening Trainer')).toBeVisible();
    await expect(page.getByText('★ Featured Opening Packs')).toBeVisible();
    await expect(page.getByText("★ Queen's Gambit")).toBeVisible();
    await expect(page.getByText('★ Modern / Pirc Defense')).toBeVisible();
    await expect(page.getByText('★ Italian Game')).toBeVisible();
    await expect(page.getByText('★ Caro-Kann Defense')).toBeVisible();
    await expect(page.getByRole('button', { name: '▶ Start Opening' })).toBeVisible();
    await expect(page.getByText('Request an Opening')).toBeVisible();
    await expect(page.getByPlaceholder('Search openings…')).toBeVisible();
  });
});

// ── Start Opening disabled ────────────────────────────────────────────────────

test.describe('Start Opening button state', () => {
  test('▶ Start Opening is disabled before any selection', async ({ page }) => {
    await gotoSetup(page);
    await expect(page.getByRole('button', { name: '▶ Start Opening' })).toBeDisabled();
  });

  test('▶ Start Opening is enabled after selecting a featured pack', async ({ page }) => {
    await gotoSetup(page);
    await page.getByText("★ Queen's Gambit").click();
    await expect(page.getByRole('button', { name: '▶ Start Opening' })).toBeEnabled();
  });
});

// ── Color badges on pack cards ──────────────────────────────────────────────

test.describe('Color badges', () => {
  test("Queen's Gambit card shows 'Play as ♔ White' badge", async ({ page }) => {
    await gotoSetup(page);
    await expect(page.getByText('Play as ♔ White').first()).toBeVisible();
  });

  test("Modern / Pirc Defense card shows 'Play as ♚ Black' badge", async ({ page }) => {
    await gotoSetup(page);
    await expect(page.getByText('Play as ♚ Black').first()).toBeVisible();
  });

  test("Italian Game card shows 'Play as ♔ White' badge", async ({ page }) => {
    await gotoSetup(page);
    // There are two "Play as White" badges (QG and Italian Game)
    const whiteBadges = page.getByText('Play as ♔ White');
    await expect(whiteBadges).toHaveCount(2);
  });

  test("Caro-Kann Defense card shows 'Play as ♚ Black' badge", async ({ page }) => {
    await gotoSetup(page);
    // There are two "Play as Black" badges (MD and Caro-Kann)
    const blackBadges = page.getByText('Play as ♚ Black');
    await expect(blackBadges).toHaveCount(2);
  });
});

// ── Rating slider ────────────────────────────────────────────────────────────

test.describe('Rating slider', () => {
  test('default rating displays as 1600', async ({ page }) => {
    await gotoSetup(page);
    // The rating value is shown as a bold span below the slider
    await expect(page.getByText('1600')).toBeVisible();
  });

  test('slider has correct min and max attributes', async ({ page }) => {
    await gotoSetup(page);
    const slider = page.locator('input[type="range"]');
    await expect(slider).toHaveAttribute('min', '800');
    await expect(slider).toHaveAttribute('max', '2500');
  });

  test('changing slider updates displayed rating', async ({ page }) => {
    await gotoSetup(page);
    const slider = page.locator('input[type="range"]');
    await slider.fill('2000');
    await expect(page.getByText('2000')).toBeVisible();
  });

  test('slider accepts extreme values 800 and 2500', async ({ page }) => {
    await gotoSetup(page);
    const slider = page.locator('input[type="range"]');
    await slider.fill('800');
    await expect(page.getByText('800').first()).toBeVisible();
    await slider.fill('2500');
    await expect(page.getByText('2500').first()).toBeVisible();
  });
});

// ── Featured packs ────────────────────────────────────────────────────────────

test.describe('Featured packs', () => {
  test("clicking Queen's Gambit card enables Start Opening and shows pack description", async ({ page }) => {
    await gotoSetup(page);
    await page.getByText("★ Queen's Gambit").click();
    await expect(page.getByRole('button', { name: '▶ Start Opening' })).toBeEnabled();
    await expect(page.getByText(/Queen's Gambit Pack/)).toBeVisible();
  });

  test('clicking Modern / Pirc Defense card enables Start Opening and shows pack description', async ({ page }) => {
    await gotoSetup(page);
    await page.getByText('★ Modern / Pirc Defense').click();
    await expect(page.getByRole('button', { name: '▶ Start Opening' })).toBeEnabled();
    await expect(page.getByText(/Modern \/ Pirc Defense Pack/)).toBeVisible();
  });

  test('clicking Italian Game card enables Start Opening and shows pack description', async ({ page }) => {
    await gotoSetup(page);
    await page.getByText('★ Italian Game').click();
    await expect(page.getByRole('button', { name: '▶ Start Opening' })).toBeEnabled();
    await expect(page.getByText(/Italian Game Pack/)).toBeVisible();
  });

  test('clicking Caro-Kann Defense card enables Start Opening and shows pack description', async ({ page }) => {
    await gotoSetup(page);
    await page.getByText('★ Caro-Kann Defense').click();
    await expect(page.getByRole('button', { name: '▶ Start Opening' })).toBeEnabled();
    await expect(page.getByText(/Caro-Kann Defense Pack/)).toBeVisible();
  });
});

// ── Curated search ────────────────────────────────────────────────────────────

test.describe('Curated search', () => {
  test('search list is hidden when input is empty', async ({ page }) => {
    await gotoSetup(page);
    // No dropdown visible with empty search
    await expect(page.getByText('Not yet available')).not.toBeVisible();
    await expect(page.getByText('📩 Request something else')).not.toBeVisible();
  });

  test('typing shows curated list filtered by query', async ({ page }) => {
    await gotoSetup(page);
    await page.getByPlaceholder('Search openings…').fill('sic');
    await expect(page.getByText('Sicilian Defense')).toBeVisible();
    await expect(page.getByText('Not yet available')).toBeVisible();
    await expect(page.getByText('📩 Request something else')).toBeVisible();
    // Queen's Gambit should not appear for "sic"
    await expect(page.getByText('★ Queen\'s GambitAvailable')).not.toBeVisible();
  });

  test('typing "queen" shows available Queen\'s Gambit with Available badge', async ({ page }) => {
    await gotoSetup(page);
    await page.getByPlaceholder('Search openings…').fill('queen');
    // The dropdown row should show "★ Queen's Gambit" with "Available"
    await expect(page.getByText('Available')).toBeVisible();
  });

  test('selecting available opening from dropdown selects the featured pack', async ({ page }) => {
    await gotoSetup(page);
    await page.getByPlaceholder('Search openings…').fill('queen');
    // Click the available Queen's Gambit dropdown row
    await page.locator('div[style*="overflow-y:auto"] div[style*="cursor:pointer"]').first().click();
    // Featured pack should be selected, Start Opening enabled
    await expect(page.getByRole('button', { name: '▶ Start Opening' })).toBeEnabled();
    await expect(page.getByText(/Queen's Gambit Pack/)).toBeVisible();
    // Search text should be cleared
    await expect(page.getByPlaceholder('Search openings…')).toHaveValue('');
  });

  test('selecting unavailable opening shows request panel', async ({ page }) => {
    await gotoSetup(page);
    await page.getByPlaceholder('Search openings…').fill('sic');
    await page.getByText('Sicilian Defense').click();
    // Request panel should appear
    await expect(page.getByText('Sicilian Defense is not yet available.')).toBeVisible();
    // Either the submit button (endpoint configured) or "closed" message (no endpoint)
    const hasButton = await page.getByRole('button', { name: '📩 Request this opening' }).isVisible().catch(() => false);
    if (hasButton) {
      await expect(page.getByText(/request\(s\) remaining this week/)).toBeVisible();
    } else {
      await expect(page.getByText(/temporarily closed/)).toBeVisible();
    }
    // Start Opening should still be disabled (Request, not Group)
    await expect(page.getByRole('button', { name: '▶ Start Opening' })).toBeDisabled();
  });

  test('searching after selecting a pack clears the selection', async ({ page }) => {
    await gotoSetup(page);
    await page.getByText("★ Queen's Gambit").click();
    await expect(page.getByRole('button', { name: '▶ Start Opening' })).toBeEnabled();
    // Typing in search clears selection
    await page.getByPlaceholder('Search openings…').fill('ruy');
    await expect(page.getByRole('button', { name: '▶ Start Opening' })).toBeDisabled();
  });

  test('📩 Request something else opens free-text panel', async ({ page }) => {
    await gotoSetup(page);
    await page.getByPlaceholder('Search openings…').fill('x');
    await page.getByText('📩 Request something else').click();
    // Free-text panel should appear
    await expect(page.getByText("Describe the opening you'd like added:")).toBeVisible();
    await expect(page.getByPlaceholder(/Scandinavian Defense/)).toBeVisible();
    // Either the remaining count (endpoint configured) or "closed" message (no endpoint)
    const hasRemaining = await page.getByText(/request\(s\) remaining this week/).isVisible().catch(() => false);
    if (!hasRemaining) {
      await expect(page.getByText(/temporarily closed/)).toBeVisible();
    }
  });
});
