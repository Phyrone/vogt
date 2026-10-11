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

	await page.getByRole('button', { name: 'Add existing folder' }).click();
	await expect(page.getByRole('region', { name: 'History' })).toBeVisible();
});

test('a repository that is gone says so', async ({ page }) => {
	await page.goto('/workspaces/ws-personal/repos/nope/history');
	await expect(page.getByRole('heading', { level: 1 })).toHaveText(
		'This repository is no longer in the workspace'
	);
});

test('switches between the worktrees of a repository', async ({ page }) => {
	await page.goto('/workspaces/ws-personal/repos/repo-dotfiles/history');
	await expect(page.locator('summary', { hasText: 'Worktree:' })).toHaveCount(0);

	await page.goto('/workspaces/ws-personal/repos/repo-vogt/history');
	const switcher = page.locator('summary', { hasText: 'Worktree:' });
	await expect(switcher).toContainText('vogt');
	await switcher.click();
	await page.getByRole('link', { name: 'vogt-graph feat/commit-graph' }).click();
	await expect(switcher).toContainText('vogt-graph');
	await expect(page.getByRole('region', { name: 'History' })).toBeVisible();
});

test('creates a new repository in the workspace', async ({ page }) => {
	await page.goto('/workspaces/ws-empty');
	await page.getByRole('button', { name: 'New repository' }).click();
	const name = page.getByLabel('Name of the new repository');
	await expect(name).toBeFocused();
	await expect(page.getByRole('button', { name: 'Create' })).toBeDisabled();

	await name.fill('playground');
	await name.press('Enter');
	await expect(page.getByRole('heading', { level: 1, name: 'playground' })).toBeVisible();
	await expect(page.getByRole('region', { name: 'History' })).toBeVisible();
});
