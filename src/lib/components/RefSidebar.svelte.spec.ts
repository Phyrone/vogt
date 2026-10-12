import { expect, test } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';
import type { ResolvedPathname } from '$app/types';
import RefSidebar from './RefSidebar.svelte';

test('stashes with the same message are each listed', async () => {
	await render(RefSidebar, {
		props: {
			lines: [],
			headLine: 'main',
			ahead: 0,
			stashes: ['On main: wip', 'On main: wip'],
			tags: [],
			picked: [],
			settingsHref: '/settings' as ResolvedPathname
		}
	});

	await expect.element(page.getByText('On main: wip').first()).toBeInTheDocument();
	expect(page.getByText('On main: wip').elements()).toHaveLength(2);
});
