import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';

interface KeybindEvent {
	id: string;
}

type KeybindHandler = () => void | Promise<void>;

const keybindHandlers = new Map<string, KeybindHandler>();
let eventUnlisten: UnlistenFn | null = null;
let clipboardUnlisten: UnlistenFn | null = null;

// Check if an input element is currently focused
function isInputFocused(): boolean {
	const activeElement = document.activeElement;
	console.log('[Clipboard] Active element:', {
		element: activeElement,
		tagName: activeElement?.tagName,
		type: (activeElement as HTMLInputElement)?.type,
		contenteditable: activeElement?.getAttribute('contenteditable')
	});

	if (!activeElement) {
		console.log('[Clipboard] No active element');
		return false;
	}

	const tagName = activeElement.tagName.toLowerCase();
	if (tagName === 'input' || tagName === 'textarea' || tagName === 'select') {
		console.log('[Clipboard] Input element focused:', tagName);
		return true;
	}

	// Check for contenteditable
	if (activeElement.getAttribute('contenteditable') === 'true') {
		console.log('[Clipboard] Contenteditable element focused');
		return true;
	}

	console.log('[Clipboard] Non-input element focused:', tagName);
	return false;
}

// Execute native clipboard operation (for text inputs)
function executeNativeClipboard(action: 'copy' | 'cut' | 'paste'): void {
	console.log(`[Clipboard] Executing native ${action} operation`);
	try {
		// Use execCommand for compatibility (deprecated but still works)
		const result = document.execCommand(action);
		console.log(`[Clipboard] execCommand('${action}') result:`, result);
	} catch (err) {
		console.error(`[Clipboard] Failed to execute native ${action}:`, err);
	}
}

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

	// Listen for clipboard actions from native menu
	clipboardUnlisten = await listen<string>('clipboard-action', async (event) => {
		const action = event.payload as 'copy' | 'cut' | 'paste';
		console.log(`[Clipboard] Received clipboard-action event:`, action);

		// Check if an input is focused
		if (isInputFocused()) {
			// Execute native browser clipboard operation
			console.log('[Clipboard] Input focused, executing native operation');
			executeNativeClipboard(action);
		} else {
			// Trigger file operation via keybind system
			const keybindId = `explorer.${action}`;
			console.log('[Clipboard] No input focused, triggering file operation:', keybindId);
			const handler = keybindHandlers.get(keybindId);
			if (handler) {
				try {
					await handler();
					console.log(`[Clipboard] File operation ${keybindId} completed`);
				} catch (err) {
					console.error(`[Clipboard] Handler error for ${keybindId}:`, err);
				}
			} else {
				console.warn(`[Clipboard] No handler registered for ${keybindId}`);
			}
		}
	});

	console.log('[Clipboard] Action listener initialized');

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

	if (clipboardUnlisten) {
		clipboardUnlisten();
		clipboardUnlisten = null;
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
