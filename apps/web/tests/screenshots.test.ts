import { test } from '@playwright/test';

test('dark screenshot', async ({ page }) => {
	await page.emulateMedia({ colorScheme: 'dark' });
	await page.goto('/');
	await page.screenshot({ path: 'screenshots/overview-dark.png', fullPage: true });
});

test('light screenshot', async ({ page }) => {
	await page.emulateMedia({ colorScheme: 'light' });
	await page.goto('/');
	await page.screenshot({ path: 'screenshots/overview-light.png', fullPage: true });
});
