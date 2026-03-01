import { test, expect } from '@playwright/test';
import { gotoSetup, waitForGameScreen } from './helpers';

test.describe('Navigation', () => {
  test('Start Opening (QG pack) → game screen shows move counter and hides setup', async ({ page }) => {
    await gotoSetup(page);
    await page.getByText("★ Queen's Gambit").click();
    await page.getByRole('button', { name: '▶ Start Opening' }).click();
    await waitForGameScreen(page);

    // Game screen buttons present
    await expect(page.getByRole('button', { name: 'New Game' })).toBeVisible();
    await expect(page.getByRole('button', { name: '↩ Undo' })).toBeVisible();
    await expect(page.getByRole('button', { name: '← Setup' })).toBeVisible();

    // Setup screen elements gone
    await expect(page.getByText('★ Featured Opening Packs')).not.toBeVisible();

    // Move counter visible
    await expect(page.getByText(/move \d+\/\d+/)).toBeVisible();
  });

  test('← Setup returns to setup screen', async ({ page }) => {
    await gotoSetup(page);
    await page.getByText("★ Queen's Gambit").click();
    await page.getByRole('button', { name: '▶ Start Opening' }).click();
    await waitForGameScreen(page);
    await page.getByRole('button', { name: '← Setup' }).click();

    // Setup screen restored
    await expect(page.getByText('★ Featured Opening Packs')).toBeVisible();
    await expect(page.getByText("★ Queen's Gambit")).toBeVisible();
  });

  test('round-trip works twice (Setup → Game → Setup → Game)', async ({ page }) => {
    await gotoSetup(page);

    // First trip
    await page.getByText("★ Queen's Gambit").click();
    await page.getByRole('button', { name: '▶ Start Opening' }).click();
    await waitForGameScreen(page);
    await page.getByRole('button', { name: '← Setup' }).click();
    await expect(page.getByText('★ Featured Opening Packs')).toBeVisible();

    // Second trip
    await page.getByText("★ Queen's Gambit").click();
    await page.getByRole('button', { name: '▶ Start Opening' }).click();
    await waitForGameScreen(page);
    await expect(page.getByRole('button', { name: 'New Game' })).toBeVisible();
  });

  test('opening mode shows ECO code and name in header', async ({ page }) => {
    await gotoSetup(page);
    await page.getByText("★ Queen's Gambit").click();
    await page.getByRole('button', { name: '▶ Start Opening' }).click();
    await waitForGameScreen(page);

    // QG pack randomly picks a variation — verify any ECO code appears
    await expect(page.getByText(/[A-E]\d{2}/)).toBeVisible();
    await expect(page.getByText(/move \d+\/\d+/)).toBeVisible();
  });

  test('MD as Black: game starts with computer thinking', async ({ page }) => {
    await gotoSetup(page);
    await page.getByText('★ Modern / Pirc Defense').click();
    await page.getByRole('button', { name: '▶ Start Opening' }).click();
    await waitForGameScreen(page);

    // Computer (White) should be thinking first
    await expect(page.getByText('Computer thinking (White)')).toBeVisible();
    // Then turn passes to Black
    await expect(page.getByText(/Your turn \(Black\)/)).toBeVisible({ timeout: 8_000 });
  });
});
