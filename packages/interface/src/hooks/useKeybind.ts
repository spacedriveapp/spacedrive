/**
 * useKeybind Hook
 *
 * Register a handler for a keybind from the unified registry.
 * Automatically handles platform detection and cleanup.
 */

import { useEffect, useRef } from 'react';
import { usePlatform } from '../platform';
import type { KeybindId } from '../util/keybinds/registry';
import { getKeybind } from '../util/keybinds/registry';
import { getComboForPlatform, getCurrentPlatform, toTauriAccelerator } from '../util/keybinds/platform';
import { getWebListener } from '../util/keybinds/listener';

export type KeybindHandler = () => void | Promise<void>;

export interface UseKeybindOptions {
	/** Whether the keybind is currently active (default: true) */
	enabled?: boolean;
}

/**
 * Register a keybind handler
 *
 * @param keybindId - Type-safe keybind ID from registry (e.g., 'explorer.copy')
 * @param handler - Function to call when keybind is triggered
 * @param options - Optional configuration
 *
 * @example
 * ```tsx
 * useKeybind('explorer.copy', () => {
 *   console.log('Copy triggered!');
 * });
 *
 * // With enabled toggle
 * useKeybind('explorer.paste', handlePaste, { enabled: hasSelection });
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

	// Keep handler ref up to date without triggering effect re-runs
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

		// Check if Tauri platform with keybind registration capability
		if (
			platform.platform === 'tauri' &&
			'registerKeybind' in platform &&
			typeof platform.registerKeybind === 'function' &&
			'unregisterKeybind' in platform &&
			typeof platform.unregisterKeybind === 'function'
		) {
			// Register with Tauri global shortcuts
			const accelerator = toTauriAccelerator(combo, currentPlatform);

			platform.registerKeybind(keybindId, accelerator, wrappedHandler);

			return () => {
				if (platform.unregisterKeybind) {
					platform.unregisterKeybind(keybindId);
				}
			};
		} else {
			// Use web listener (default)
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

export default useKeybind;
