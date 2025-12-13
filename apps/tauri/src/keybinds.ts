import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';

interface KeybindEvent {
	id: string;
}

type KeybindHandler = () => void | Promise<void>;

const keybindHandlers = new Map<string, KeybindHandler>();
let eventUnlisten: UnlistenFn | null = null;

// Initialize Tauri keybind listener
export async function initializeKeybindHandler(): Promise<void> {
	// Only initialize once
	if (eventUnlisten !== null) return;

	// Listen for keybind events from Rust
	eventUnlisten = await listen<KeybindEvent>('keybind-triggered', async (event) => {
		const handler = keybindHandlers.get(event.payload.id);
		if (handler) {
			try {
				await handler();
			} catch (err) {
				console.error(`[Keybind] Handler error for ${event.payload.id}:`, err);
			}
		}
	});

	console.log('[Keybind] Handler initialized');
}

// Register a keybind with Tauri
export async function registerTauriKeybind(
	id: string,
	accelerator: string,
	handler: KeybindHandler
): Promise<void> {
	keybindHandlers.set(id, handler);

	try {
		await invoke('register_keybind', {
			id,
			accelerator
		});
		console.log(`[Keybind] Registered: ${id} (${accelerator})`);
	} catch (error) {
		console.error(`[Keybind] Failed to register ${id}:`, error);
		// Keep the handler registered for web fallback
	}
}

// Unregister a keybind
export async function unregisterTauriKeybind(id: string): Promise<void> {
	keybindHandlers.delete(id);

	try {
		await invoke('unregister_keybind', { id });
		console.log(`[Keybind] Unregistered: ${id}`);
	} catch (error) {
		console.error(`[Keybind] Failed to unregister ${id}:`, error);
	}
}

// Cleanup function
export async function cleanupKeybindHandler(): Promise<void> {
	if (eventUnlisten) {
		eventUnlisten();
		eventUnlisten = null;
	}

	// Unregister all keybinds
	const ids = Array.from(keybindHandlers.keys());
	for (const id of ids) {
		try {
			await invoke('unregister_keybind', { id });
		} catch {
			// Ignore errors during cleanup
		}
	}
	keybindHandlers.clear();

	console.log('[Keybind] Handler cleaned up');
}

// Initialize keybind handler on window global (same pattern as context menu)
export function initializeKeybindGlobal(): void {
	if (!window.__SPACEDRIVE__) {
		(window as any).__SPACEDRIVE__ = {};
	}

	window.__SPACEDRIVE__.registerKeybind = registerTauriKeybind;
	window.__SPACEDRIVE__.unregisterKeybind = unregisterTauriKeybind;

	// Initialize the event listener
	initializeKeybindHandler().catch(console.error);

	console.log('[Keybind] Global handlers initialized');
}
