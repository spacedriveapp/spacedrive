/**
 * Web Keyboard Listener
 *
 * Singleton class that manages keyboard event handling for the web platform.
 * This provides the fallback keyboard handling when Tauri global shortcuts are not available.
 */

import type { KeyCombo, KeybindScope, Platform } from './types';
import { getCurrentPlatform, matchesKeyCombo } from './platform';

export type KeybindHandler = () => void | Promise<void>;

interface RegisteredKeybind {
	combo: KeyCombo;
	handler: KeybindHandler;
	scope: KeybindScope;
	preventDefault: boolean;
}

/**
 * Singleton listener for web platform keyboard events.
 * Manages registration and scoping of keybinds.
 */
class WebKeybindListener {
	private registeredKeybinds = new Map<string, RegisteredKeybind>();
	private activeScopes = new Set<KeybindScope>(['global']);
	private platform: Platform;
	private boundHandleKeyDown: (e: KeyboardEvent) => void;

	constructor() {
		this.platform = getCurrentPlatform();
		this.boundHandleKeyDown = this.handleKeyDown.bind(this);

		if (typeof window !== 'undefined') {
			window.addEventListener('keydown', this.boundHandleKeyDown);
		}
	}

	/**
	 * Register a keybind handler.
	 * The handler will be called when the key combo is pressed and the scope is active.
	 */
	register(
		id: string,
		combo: KeyCombo,
		handler: KeybindHandler,
		scope: KeybindScope,
		preventDefault = false
	): void {
		this.registeredKeybinds.set(id, {
			combo,
			handler,
			scope,
			preventDefault,
		});
	}

	/**
	 * Unregister a keybind by its ID.
	 */
	unregister(id: string): void {
		this.registeredKeybinds.delete(id);
	}

	/**
	 * Activate a keybind scope.
	 * Keybinds in this scope will now respond to keyboard events.
	 */
	pushScope(scope: KeybindScope): void {
		this.activeScopes.add(scope);
	}

	/**
	 * Deactivate a keybind scope.
	 * Keybinds in this scope will no longer respond to keyboard events.
	 */
	popScope(scope: KeybindScope): void {
		this.activeScopes.delete(scope);
	}

	/**
	 * Check if a scope is currently active.
	 */
	isScopeActive(scope: KeybindScope): boolean {
		return scope === 'global' || this.activeScopes.has(scope);
	}

	/**
	 * Get all currently active scopes.
	 */
	getActiveScopes(): KeybindScope[] {
		return Array.from(this.activeScopes);
	}

	/**
	 * Handle keyboard events.
	 * Checks all registered keybinds and triggers the first matching handler.
	 */
	private handleKeyDown(e: KeyboardEvent): void {
		// Skip if user is typing in an input field
		const target = e.target as HTMLElement;
		if (this.shouldIgnoreEvent(target)) {
			return;
		}

		// Find matching keybind
		for (const [, keybind] of this.registeredKeybinds) {
			// Check if scope is active
			if (!this.isScopeActive(keybind.scope)) continue;

			// Check if key combo matches
			if (matchesKeyCombo(e, keybind.combo, this.platform)) {
				if (keybind.preventDefault) {
					e.preventDefault();
					e.stopPropagation();
				}

				// Call handler asynchronously to avoid blocking
				Promise.resolve(keybind.handler()).catch((err) => {
					console.error(`Keybind handler error:`, err);
				});
				return;
			}
		}
	}

	/**
	 * Determine if a keyboard event should be ignored.
	 * Returns true if the user is typing in a text input.
	 */
	private shouldIgnoreEvent(target: HTMLElement): boolean {
		const tagName = target.tagName.toLowerCase();

		// Ignore if typing in input fields
		if (tagName === 'input' || tagName === 'textarea') {
			return true;
		}

		// Ignore if typing in contenteditable elements
		if (target.isContentEditable) {
			return true;
		}

		return false;
	}

	/**
	 * Clean up the listener and remove all event handlers.
	 */
	destroy(): void {
		if (typeof window !== 'undefined') {
			window.removeEventListener('keydown', this.boundHandleKeyDown);
		}
		this.registeredKeybinds.clear();
		this.activeScopes.clear();
		this.activeScopes.add('global');
	}
}

// Global singleton instance
let listenerInstance: WebKeybindListener | null = null;

/**
 * Get the global web keybind listener instance.
 * Creates the instance lazily on first access.
 */
export function getWebListener(): WebKeybindListener {
	if (!listenerInstance) {
		listenerInstance = new WebKeybindListener();
	}
	return listenerInstance;
}

/**
 * Destroy the global web keybind listener instance.
 * Call this during cleanup (e.g., in tests or when unmounting the app).
 */
export function destroyWebListener(): void {
	if (listenerInstance) {
		listenerInstance.destroy();
		listenerInstance = null;
	}
}
