import { isTauri } from '@tauri-apps/api/core';
import { Menu, MenuItem, PredefinedMenuItem, Submenu } from '@tauri-apps/api/menu';
import type { PredefinedMenuItemOptions } from '@tauri-apps/api/menu';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { goto } from '$app/navigation';
import { resolve } from '$app/paths';
import { m } from '#lib/paraglide/messages.js';
import { appVersion, openExternal } from './settings/system.ts';
import { windowChrome } from './window.svelte.ts';

const REPOSITORY = 'https://github.com/Phyrone/vogt';

function separator(): Promise<PredefinedMenuItem> {
	return PredefinedMenuItem.new({ item: 'Separator' });
}

function predefined(item: PredefinedMenuItemOptions['item']) {
	return PredefinedMenuItem.new({ item });
}

/**
 * The native window menu. macOS gets the app menu in the system menu bar; Windows
 * and Linux get a menu bar on the window. Settings opens the app's settings page;
 * a repository's settings are reached from its sidebar or the title-bar dropdown.
 * The interface rebuilds it when the language changes.
 */
export async function installMenu(): Promise<void> {
	if (!isTauri()) return;

	const settings = await MenuItem.new({
		id: 'settings',
		text: m['menu.settings'](),
		accelerator: 'CmdOrCtrl+,',
		action: () => void goto(resolve('/settings'))
	});
	const github = await MenuItem.new({
		id: 'github',
		text: m['menu.github'](),
		action: () => void openExternal(REPOSITORY)
	});

	const menu = await Menu.new({
		items:
			windowChrome === 'mac' ? await macMenu(settings, github) : await desktopMenu(settings, github)
	});
	if (windowChrome === 'mac') await menu.setAsAppMenu();
	else await menu.setAsWindowMenu(getCurrentWindow());
}

/** The menu macOS expects: an app menu, Edit, View, Window and Help in the system menu bar. */
async function macMenu(settings: MenuItem, github: MenuItem) {
	const app = await Submenu.new({
		text: 'VOGT',
		items: [
			await predefined({
				About: { name: 'VOGT', version: await appVersion() }
			}),
			settings,
			await separator(),
			await predefined('Services'),
			await separator(),
			await predefined('Hide'),
			await predefined('HideOthers'),
			await predefined('ShowAll'),
			await separator(),
			await predefined('Quit')
		]
	});
	const edit = await Submenu.new({
		text: m['menu.edit'](),
		items: [
			await predefined('Undo'),
			await predefined('Redo'),
			await separator(),
			await predefined('Cut'),
			await predefined('Copy'),
			await predefined('Paste'),
			await predefined('SelectAll')
		]
	});
	const view = await Submenu.new({
		text: m['menu.view'](),
		items: [await predefined('Fullscreen')]
	});
	const window = await Submenu.new({
		text: m['menu.window'](),
		items: [
			await predefined('Minimize'),
			await predefined('Maximize'),
			await separator(),
			await predefined('CloseWindow')
		]
	});
	await window.setAsWindowsMenuForNSApp();
	const help = await Submenu.new({ text: m['menu.help'](), items: [github] });
	await help.setAsHelpMenuForNSApp();
	return [app, edit, view, window, help];
}

/** The window menu bar for Windows and Linux; the window's own controls are the window items. */
async function desktopMenu(settings: MenuItem, github: MenuItem) {
	const appWindow = getCurrentWindow();
	const file = await Submenu.new({
		text: m['menu.file'](),
		items: [
			settings,
			await separator(),
			await MenuItem.new({
				id: 'exit',
				text: m['menu.exit'](),
				action: () => void appWindow.close()
			})
		]
	});
	const edit = await Submenu.new({
		text: m['menu.edit'](),
		items: [
			await predefined('Cut'),
			await predefined('Copy'),
			await predefined('Paste'),
			await predefined('SelectAll')
		]
	});
	const window = await Submenu.new({
		text: m['menu.window'](),
		items: [
			await MenuItem.new({
				id: 'minimize',
				text: m['common.minimize'](),
				action: () => void appWindow.minimize()
			}),
			await MenuItem.new({
				id: 'maximize',
				text: m['common.maximize'](),
				action: () => void appWindow.toggleMaximize()
			}),
			await MenuItem.new({
				id: 'close',
				text: m['menu.close_window'](),
				action: () => void appWindow.close()
			})
		]
	});
	const help = await Submenu.new({ text: m['menu.help'](), items: [github] });
	return [file, edit, window, help];
}
