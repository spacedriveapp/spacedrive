/**
 * useKeybind Hook
 *
 * Platform-agnostic hook for registering keyboard shortcuts.
 * Uses Tauri global shortcuts on native platforms and falls back to
 * JavaScript keyboard listeners on web.
 */

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
}

/**
 * Register a keybind handler for a predefined keybind.
 *
 * This hook provides platform abstraction:
 * - On Tauri: Uses native global shortcuts (if available)
 * - On Web: Uses JavaScript keyboard event listeners
 *
 * @param keybindId - Type-safe keybind ID from the registry
 * @param handler - Function to call when the keybind is triggered
 * @param options - Optional configuration
 *
 * @example
 * ```tsx
 * useKeybind('explorer.copy', async () => {
 *   await copySelectedFiles();
 * });
 *
 * // Conditionally enable
 * useKeybind('explorer.paste', handlePaste, { enabled: hasClipboardContent });
 * ```
 */
export function useKeybind(
	keybindId: KeybindId,
	handler: KeybindHandler,
	options: UseKeybindOptions = {}
): void {
	const { enabled = true } = options;
	const platform = usePlatform();
	const handlerRef = useRef(handler);

	// Keep handler ref up to date without re-registering
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

		// Check if we can use Tauri global shortcuts
		const hasTauriKeybinds =
			platform.platform === 'tauri' &&
			typeof (platform as any).registerKeybind === 'function' &&
			typeof (platform as any).unregisterKeybind === 'function';

		if (hasTauriKeybinds) {
			// Register with Tauri
			const accelerator = toTauriAccelerator(combo, currentPlatform);

			(platform as any).registerKeybind(keybindId, accelerator, wrappedHandler);

			return () => {
				(platform as any).unregisterKeybind(keybindId);
			};
		} else {
			// Fallback to web listener
			const listener = getWebListener();

			listener.register(keybindId, combo, wrappedHandler, keybind.scope, keybind.preventDefault ?? false);

			return () => {
				listener.unregister(keybindId);
			};
		}
	}, [keybindId, enabled, platform]);
}
