import { useEffect, useRef } from 'react';
import { usePlatform } from '../platform';
import type { KeybindId } from '../util/keybinds/registry';
import { getKeybind } from '../util/keybinds/registry';
import { getComboForPlatform, getCurrentPlatform, toTauriAccelerator } from '../util/keybinds/platform';
import { getWebListener } from '../util/keybinds/listener';

export type KeybindHandler = () => void | Promise<void>;

export interface UseKeybindOptions {
	enabled?: boolean;
}

/**
 * Register a keybind handler
 *
 * @param keybindId - Type-safe keybind ID from registry
 * @param handler - Function to call when keybind is triggered
 * @param options - Optional configuration
 */
export function useKeybind(keybindId: KeybindId, handler: KeybindHandler, options: UseKeybindOptions = {}) {
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
			console.warn(`Keybind not found: ${keybindId}`);
			return;
		}

		const currentPlatform = getCurrentPlatform();
		const combo = getComboForPlatform(keybind.combo, currentPlatform);

		const wrappedHandler = async () => {
			await handlerRef.current();
		};

		// Check if platform has native keybind support
		const platformWithKeybinds = platform as typeof platform & {
			registerKeybind?(id: string, accelerator: string, handler: () => void | Promise<void>): Promise<void>;
			unregisterKeybind?(id: string): Promise<void>;
		};

		if (
			platform.platform === 'tauri' &&
			platformWithKeybinds.registerKeybind &&
			platformWithKeybinds.unregisterKeybind
		) {
			// Register with Tauri (future support)
			const accelerator = toTauriAccelerator(combo, currentPlatform);
			platformWithKeybinds.registerKeybind(keybindId, accelerator, wrappedHandler);

			return () => {
				platformWithKeybinds.unregisterKeybind!(keybindId);
			};
		} else {
			// Fallback to web listener (current implementation)
			const listener = getWebListener();

			listener.register(keybindId, combo, wrappedHandler, keybind.scope, keybind.preventDefault ?? false);

			return () => {
				listener.unregister(keybindId);
			};
		}
	}, [keybindId, enabled, platform]);
}
