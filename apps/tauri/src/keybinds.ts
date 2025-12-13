/**
 * Tauri Keybind Integration
 *
 * This module provides native keyboard shortcut handling for the Tauri desktop app.
 * It mirrors the context menu pattern - platform-agnostic definitions in @sd/interface,
 * with Tauri-specific implementation here.
 *
 * Note: Tauri 2.x uses keyboard shortcuts at the menu level (accelerators) rather than
 * global shortcuts. For app-wide keybinds, we primarily rely on the web listener,
 * but this module provides the infrastructure for future global shortcut support.
 */

export type KeybindHandler = () => void | Promise<void>;

const keybindHandlers = new Map<string, KeybindHandler>();

// Spacedrive global interface for keybinds
interface SpacedriveGlobal {
	showContextMenu?(items: unknown[], position: { x: number; y: number }): Promise<void>;
	clipboard?: {
		operation: 'copy' | 'cut';
		files: unknown[];
		sourcePath: unknown;
	};
	registerKeybind?(id: string, accelerator: string, handler: KeybindHandler): void;
	unregisterKeybind?(id: string): void;
	triggerKeybind?(id: string): Promise<void>;
}

declare global {
	interface Window {
		__SPACEDRIVE__?: SpacedriveGlobal;
	}
}

/**
 * Register a keybind handler
 *
 * This stores the handler for a keybind ID. The actual keyboard listening
 * is done by the web listener, but this provides a registration point
 * for potential future Tauri global shortcut support.
 */
export function registerKeybind(
	id: string,
	_accelerator: string,
	handler: KeybindHandler
): void {
	keybindHandlers.set(id, handler);
}

/**
 * Unregister a keybind handler
 */
export function unregisterKeybind(id: string): void {
	keybindHandlers.delete(id);
}

/**
 * Trigger a keybind by ID
 *
 * This can be called from Rust via events to trigger keybinds
 * registered on the TypeScript side.
 */
export async function triggerKeybind(id: string): Promise<void> {
	const handler = keybindHandlers.get(id);
	if (handler) {
		await handler();
	}
}

/**
 * Initialize the keybind handler on the window global
 *
 * This makes keybind functions available to the platform-agnostic code
 * in @sd/interface via window.__SPACEDRIVE__.
 */
export function initializeKeybindHandler(): void {
	if (!window.__SPACEDRIVE__) {
		(window as unknown as { __SPACEDRIVE__: SpacedriveGlobal }).__SPACEDRIVE__ = {} as SpacedriveGlobal;
	}

	const spacedrive = window.__SPACEDRIVE__!;
	spacedrive.registerKeybind = registerKeybind;
	spacedrive.unregisterKeybind = unregisterKeybind;
	spacedrive.triggerKeybind = triggerKeybind;

	console.log('[Tauri Keybinds] Handler initialized');
}
