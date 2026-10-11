import { expect, test } from '@playwright/test';

test('starts in a workspace and opens a repository from it', async ({ page }) => {
	await page.goto('/');
	await expect(page.getByRole('heading', { level: 1, name: 'Personal' })).toBeVisible();

	await page.getByRole('link', { name: 'dotfiles' }).click();
	await expect(page).toHaveURL(/\/workspaces\/ws-personal\/repos\/repo-dotfiles\/history$/);
	await expect(page.getByRole('heading', { level: 1, name: 'dotfiles' })).toBeVisible();
	await expect(page.getByRole('region', { name: 'History' })).toBeVisible();
});

test('adds a repository to a new workspace', async ({ page }) => {
	await page.goto('/');
	await page.locator('summary', { hasText: 'Workspace:' }).click();
	await page.getByLabel('New workspace name').fill('Clients');
	await page.getByRole('button', { name: 'Create' }).click();
	await expect(page.getByRole('heading', { level: 1, name: 'Clients' })).toBeVisible();
	await expect(page.getByText('No repositories in this workspace yet.')).toBeVisible();

	await page.getByRole('button', { name: 'Add repository' }).click();
	await expect(page.getByRole('region', { name: 'History' })).toBeVisible();
});

test('a repository that is gone says so', async ({ page }) => {
	await page.goto('/workspaces/ws-personal/repos/nope/history');
	await expect(page.getByRole('heading', { level: 1 })).toHaveText(
		'This repository is no longer in the workspace'
	);
});
