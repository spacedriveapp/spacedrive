import { useEffect, useRef } from 'react';
import { usePlatform } from '../platform';
import type { KeybindId } from '../util/keybinds/registry';
import { getKeybind } from '../util/keybinds/registry';
import {
	getComboForPlatform,
	getCurrentPlatform,
	toTauriAccelerator
} from '../util/keybinds/platform';
import { getWebListener } from '../util/keybinds/listener';

export type KeybindHandler = () => void | Promise<void>;

export interface UseKeybindOptions {
	/** Whether the keybind is currently active. Defaults to true. */
	enabled?: boolean;
}

/**
 * Register a keybind handler
 *
 * This hook provides platform abstraction for keyboard shortcuts:
 * - On web: Uses a JavaScript keydown listener
 * - On Tauri: Uses global shortcuts via the native API (if available)
 *
 * @param keybindId - Type-safe keybind ID from registry
 * @param handler - Function to call when keybind is triggered
 * @param options - Optional configuration
 *
 * @example
 * ```tsx
 * useKeybind('explorer.copy', () => {
 *   console.log('Copy triggered');
 *   copySelectedItems();
 * });
 *
 * // With enabled option
 * useKeybind('explorer.paste', () => {
 *   pasteItems();
 * }, { enabled: hasClipboardData });
 * ```
 */
export function useKeybind(
	keybindId: KeybindId,
	handler: KeybindHandler,
	options: UseKeybindOptions = {}
) {
	const { enabled = true } = options;
	const platform = usePlatform();
	const handlerRef = useRef(handler);

	// Keep handler ref up to date
	useEffect(() => {
		handlerRef.current = handler;
	}, [handler]);

	useEffect(() => {
		if (!enabled) return;

		const keybind = getKeybind(keybindId);
		if (!keybind) {
			console.warn(`[useKeybind] Keybind not found: ${keybindId}`);
			return;
		}

		const currentPlatform = getCurrentPlatform();
		const combo = getComboForPlatform(keybind.combo, currentPlatform);

		const wrappedHandler = async () => {
			await handlerRef.current();
		};

		// Check if Tauri global shortcuts are available
		const spacedrive = typeof window !== 'undefined' ? window.__SPACEDRIVE__ : undefined;
		const hasTauriKeybinds =
			platform.platform === 'tauri' && spacedrive?.registerKeybind;

		if (hasTauriKeybinds && spacedrive.registerKeybind) {
			// Register with Tauri
			const accelerator = toTauriAccelerator(combo, currentPlatform);

			spacedrive.registerKeybind(keybindId, accelerator, wrappedHandler);

			return () => {
				spacedrive.unregisterKeybind?.(keybindId);
			};
		} else {
			// Fallback to web listener
			const listener = getWebListener();

			listener.register(
				keybindId,
				combo,
				wrappedHandler,
				keybind.scope,
				keybind.preventDefault ?? false
			);

			return () => {
				listener.unregister(keybindId);
			};
		}
	}, [keybindId, enabled, platform]);
}
