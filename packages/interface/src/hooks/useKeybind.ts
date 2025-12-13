import { useEffect, useRef } from 'react';
import { usePlatform } from '../platform';
import type { KeybindId } from '../util/keybinds/registry';
import { getKeybind } from '../util/keybinds/registry';
import { getComboForPlatform, getCurrentPlatform, toTauriAccelerator } from '../util/keybinds/platform';
import { getWebListener } from '../util/keybinds/listener';

export type KeybindHandler = () => void | Promise<void>;

export interface UseKeybindOptions {
	/** Whether the keybind is enabled. Defaults to true. */
	enabled?: boolean;
	/** Whether to ignore this keybind when an input element is focused. Defaults to true. */
	ignoreWhenInputFocused?: boolean;
}

/**
 * Register a keybind handler.
 *
 * @param keybindId - Type-safe keybind ID from registry
 * @param handler - Function to call when keybind is triggered
 * @param options - Optional configuration
 *
 * @example
 * ```tsx
 * useKeybind('explorer.selectAll', () => {
 *   selectAll(files);
 * });
 *
 * // With options
 * useKeybind('explorer.copy', handleCopy, { enabled: selectedFiles.length > 0 });
 * ```
 */
export function useKeybind(
	keybindId: KeybindId,
	handler: KeybindHandler,
	options: UseKeybindOptions = {}
): void {
	const { enabled = true, ignoreWhenInputFocused = true } = options;
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

		// Always use web listener for consistent behavior across platforms
		// The Tauri platform can additionally register accelerators for native menu display
		const listener = getWebListener();

		listener.register(
			keybindId,
			combo,
			wrappedHandler,
			keybind.scope,
			keybind.preventDefault ?? false,
			ignoreWhenInputFocused
		);

		// If running in Tauri, also register with native side for menu accelerator display
		if (platform.platform === 'tauri' && platform.registerKeybind) {
			const accelerator = toTauriAccelerator(combo, currentPlatform);
			platform.registerKeybind(keybindId, accelerator, wrappedHandler).catch(err => {
				console.warn(`[useKeybind] Failed to register native accelerator for ${keybindId}:`, err);
			});
		}

		return () => {
			listener.unregister(keybindId);

			if (platform.platform === 'tauri' && platform.unregisterKeybind) {
				platform.unregisterKeybind(keybindId).catch(() => {
					// Ignore errors during cleanup
				});
			}
		};
	}, [keybindId, enabled, ignoreWhenInputFocused, platform]);
}
