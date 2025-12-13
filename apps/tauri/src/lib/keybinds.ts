import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';

interface KeybindEvent {
	id: string;
}

const keybindHandlers = new Map<string, () => void | Promise<void>>();

/**
 * Initialize Tauri keybind listener
 * Must be called once at app startup
 */
export async function initializeKeybindHandler(): Promise<() => void> {
	// Listen for keybind events from Rust
	const unlisten = await listen<KeybindEvent>('keybind-triggered', async (event) => {
		const handler = keybindHandlers.get(event.payload.id);
		if (handler) {
			try {
				await handler();
			} catch (error) {
				console.error(`Keybind handler error for ${event.payload.id}:`, error);
			}
		}
	});

	return unlisten;
}

/**
 * Register a keybind with Tauri
 * @param id Unique identifier for the keybind
 * @param accelerator Tauri accelerator string (e.g., "Cmd+C", "Ctrl+Shift+P")
 * @param handler Function to call when keybind is triggered
 */
export async function registerTauriKeybind(
	id: string,
	accelerator: string,
	handler: () => void | Promise<void>
): Promise<void> {
	keybindHandlers.set(id, handler);

	try {
		await invoke('register_global_keybind', {
			id,
			accelerator
		});
	} catch (error) {
		console.error(`Failed to register keybind ${id}:`, error);
		// Remove handler if registration failed
		keybindHandlers.delete(id);
		throw error;
	}
}

/**
 * Unregister a keybind
 * @param id Identifier of the keybind to unregister
 */
export async function unregisterTauriKeybind(id: string): Promise<void> {
	keybindHandlers.delete(id);

	try {
		await invoke('unregister_global_keybind', { id });
	} catch (error) {
		console.error(`Failed to unregister keybind ${id}:`, error);
	}
}

/**
 * Check if a keybind is registered
 * @param id Keybind identifier
 */
export function isKeybindRegistered(id: string): boolean {
	return keybindHandlers.has(id);
}
