/**
 * Tauri Keybind Integration
 *
 * This module provides the Tauri-specific implementation for global shortcuts.
 * It bridges the TypeScript keybind system with Tauri's native global shortcut API.
 */

import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';

export type KeybindHandler = () => void | Promise<void>;

interface KeybindEvent {
	id: string;
}

// Map of keybind IDs to their handlers
const keybindHandlers = new Map<string, KeybindHandler>();

// Track if event listener is initialized
let isInitialized = false;

/**
 * Initialize the Tauri keybind listener.
 * This sets up the event listener for keybind triggers from Rust.
 * Should be called once during app initialization.
 */
export async function initializeKeybindHandler(): Promise<void> {
	if (isInitialized) {
		return;
	}

	// Listen for keybind events from Rust
	await listen<KeybindEvent>('keybind-triggered', async (event) => {
		const handler = keybindHandlers.get(event.payload.id);
		if (handler) {
			try {
				await handler();
			} catch (error) {
				console.error(`[keybinds] Handler error for ${event.payload.id}:`, error);
			}
		}
	});

	isInitialized = true;
}

/**
 * Register a keybind with Tauri's global shortcut system.
 *
 * @param id - Unique identifier for the keybind
 * @param accelerator - Tauri accelerator string (e.g., "Cmd+C", "Ctrl+Shift+P")
 * @param handler - Function to call when the keybind is triggered
 */
export async function registerTauriKeybind(
	id: string,
	accelerator: string,
	handler: KeybindHandler
): Promise<void> {
	keybindHandlers.set(id, handler);

	try {
		await invoke('register_global_shortcut', {
			id,
			accelerator,
		});
	} catch (error) {
		console.error(`[keybinds] Failed to register keybind ${id}:`, error);
		// Don't throw - allow graceful degradation
	}
}

/**
 * Unregister a keybind from Tauri's global shortcut system.
 *
 * @param id - The keybind ID to unregister
 */
export async function unregisterTauriKeybind(id: string): Promise<void> {
	keybindHandlers.delete(id);

	try {
		await invoke('unregister_global_shortcut', { id });
	} catch (error) {
		console.error(`[keybinds] Failed to unregister keybind ${id}:`, error);
		// Don't throw - allow graceful degradation
	}
}

/**
 * Check if the Tauri keybind system is available.
 * Returns false if running in a context where Tauri is not available.
 */
export function isTauriKeybindAvailable(): boolean {
	return typeof window !== 'undefined' && '__TAURI__' in window;
}
