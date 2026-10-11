import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';

/**
 * Who draws the window controls.
 *
 * - `browser`: the page runs outside the Tauri shell, which has no window to control.
 * - `mac`: macOS overlays its own traffic lights on the top left of the page.
 * - `custom`: the window has no decorations and the interface draws the controls itself.
 */
export type WindowChrome = 'browser' | 'mac' | 'custom';

export const windowChrome: WindowChrome = !isTauri()
	? 'browser'
	: navigator.userAgent.includes('Mac')
		? 'mac'
		: 'custom';

/** Follow the window's maximized and fullscreen state. Call during component initialisation. */
export function trackWindow() {
	let maximized = $state(false);
	let fullscreen = $state(false);

	$effect(() => {
		if (windowChrome === 'browser') return;
		const appWindow = getCurrentWindow();
		const read = async () => {
			[maximized, fullscreen] = await Promise.all([
				appWindow.isMaximized(),
				appWindow.isFullscreen()
			]);
		};
		void read();
		const unlisten = appWindow.onResized(read);
		return () => void unlisten.then((stop) => stop());
	});

	return {
		get maximized() {
			return maximized;
		},
		get fullscreen() {
			return fullscreen;
		}
	};
}
