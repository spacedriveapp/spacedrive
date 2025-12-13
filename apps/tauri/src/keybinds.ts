/**
 * Tauri Keybind Integration
 *
 * Handles keybind registration with Tauri's backend.
 * Note: For now, this module provides a web-based listener since
 * Tauri v2 global shortcuts require plugin setup. The system is designed
 * to be extended with native shortcuts when needed.
 */

import { listen, type UnlistenFn } from '@tauri-apps/api/event';

interface KeybindEvent {
	id: string;
}

// Map of registered keybind handlers
const keybindHandlers = new Map<string, () => void | Promise<void>>();

// Event listener cleanup function
let eventUnlisten: UnlistenFn | null = null;

/**
 * Initialize Tauri keybind event listener
 * Called once when the app starts
 */
export async function initializeKeybindHandler(): Promise<void> {
	// Clean up any existing listener
	if (eventUnlisten) {
		eventUnlisten();
		eventUnlisten = null;
	}

	// Listen for keybind events from Rust backend
	eventUnlisten = await listen<KeybindEvent>('keybind-triggered', async (event) => {
		const handler = keybindHandlers.get(event.payload.id);
		if (handler) {
			try {
				await handler();
			} catch (error) {
				console.error(`[Keybind] Error handling ${event.payload.id}:`, error);
			}
		}
	});
}

/**
 * Register a keybind handler for Tauri
 *
 * Note: Currently, keybinds are handled via web listeners.
 * This function stores the handler for potential future native integration.
 *
 * @param id - Unique keybind identifier
 * @param accelerator - Tauri accelerator string (e.g., "Cmd+C")
 * @param handler - Function to call when keybind is triggered
 */
export async function registerTauriKeybind(
	id: string,
	accelerator: string,
	handler: () => void | Promise<void>
): Promise<void> {
	keybindHandlers.set(id, handler);

	// Note: Global shortcut registration would go here
	// For Tauri v2, this requires the global-shortcut plugin:
	//
	// import { register } from '@tauri-apps/plugin-global-shortcut';
	// await register(accelerator, () => { ... });
	//
	// For now, we rely on web-based keyboard listeners which work
	// in both web and Tauri environments.

	console.debug(`[Keybind] Registered handler for ${id} (${accelerator})`);
}

/**
 * Unregister a keybind handler
 *
 * @param id - The keybind ID to unregister
 */
export async function unregisterTauriKeybind(id: string): Promise<void> {
	keybindHandlers.delete(id);

	// Note: Global shortcut unregistration would go here
	// import { unregister } from '@tauri-apps/plugin-global-shortcut';
	// await unregister(accelerator);

	console.debug(`[Keybind] Unregistered handler for ${id}`);
}

/**
 * Cleanup all keybind handlers
 */
export async function cleanupKeybindHandlers(): Promise<void> {
	keybindHandlers.clear();

	if (eventUnlisten) {
		eventUnlisten();
		eventUnlisten = null;
	}
}

/**
 * Get all registered keybind IDs (for debugging)
 */
export function getRegisteredKeybindIds(): string[] {
	return Array.from(keybindHandlers.keys());
}
