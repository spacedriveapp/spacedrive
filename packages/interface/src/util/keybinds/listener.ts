/**
 * Unified Keybind System - Web Keyboard Listener
 *
 * Singleton keyboard event listener for web platform.
 * Handles scope-aware keybind registration and execution.
 */

import type { KeyCombo, Platform } from './types';
import { eventMatchesCombo, getCurrentPlatform } from './platform';

export type KeybindHandler = () => void | Promise<void>;

interface RegisteredKeybind {
	combo: KeyCombo;
	handler: KeybindHandler;
	scope: string;
	preventDefault: boolean;
}

/**
 * Singleton listener for web platform keyboard events.
 * Manages keybind registration, scope activation, and event matching.
 */
class WebKeybindListener {
	private registeredKeybinds = new Map<string, RegisteredKeybind>();
	private activeScopes = new Set<string>(['global']);
	private platform: Platform;
	private initialized = false;

	constructor() {
		this.platform = getCurrentPlatform();
	}

	/**
	 * Initialize the listener by attaching to window events
	 * Called lazily on first registration
	 */
	private ensureInitialized() {
		if (this.initialized) return;

		if (typeof window !== 'undefined') {
			window.addEventListener('keydown', this.handleKeyDown);
			this.initialized = true;
		}
	}

	/**
	 * Register a keybind with handler
	 */
	register(
		id: string,
		combo: KeyCombo,
		handler: KeybindHandler,
		scope: string,
		preventDefault = false
	) {
		this.ensureInitialized();

		this.registeredKeybinds.set(id, {
			combo,
			handler,
			scope,
			preventDefault
		});
	}

	/**
	 * Unregister a keybind by ID
	 */
	unregister(id: string) {
		this.registeredKeybinds.delete(id);
	}

	/**
	 * Activate a scope (make its keybinds responsive)
	 */
	pushScope(scope: string) {
		this.activeScopes.add(scope);
	}

	/**
	 * Deactivate a scope
	 */
	popScope(scope: string) {
		// Never remove global scope
		if (scope !== 'global') {
			this.activeScopes.delete(scope);
		}
	}

	/**
	 * Check if a scope is currently active
	 */
	isScopeActive(scope: string): boolean {
		return scope === 'global' || this.activeScopes.has(scope);
	}

	/**
	 * Get all currently active scopes
	 */
	getActiveScopes(): string[] {
		return Array.from(this.activeScopes);
	}

	/**
	 * Handle keydown events
	 */
	private handleKeyDown = async (e: KeyboardEvent) => {
		// Skip if user is typing in an input/textarea/contenteditable
		const target = e.target as HTMLElement;
		if (this.isInputElement(target)) {
			// Still allow certain keybinds even in inputs (Escape to close, etc.)
			// Only process keybinds that explicitly allow input focus
			// For now, let all keybinds through - specific keybinds can check this themselves
		}

		// Find and execute matching keybind
		for (const [_id, keybind] of this.registeredKeybinds) {
			// Check if scope is active
			if (!this.isScopeActive(keybind.scope)) continue;

			// Check if key combo matches
			if (eventMatchesCombo(e, keybind.combo, this.platform)) {
				if (keybind.preventDefault) {
					e.preventDefault();
					e.stopPropagation();
				}

				try {
					await keybind.handler();
				} catch (error) {
					console.error(`[Keybind] Error executing handler for ${_id}:`, error);
				}
				return;
			}
		}
	};

	/**
	 * Check if element is an input-like element
	 */
	private isInputElement(element: HTMLElement): boolean {
		const tagName = element.tagName.toLowerCase();
		return (
			tagName === 'input' ||
			tagName === 'textarea' ||
			tagName === 'select' ||
			element.isContentEditable
		);
	}

	/**
	 * Update the platform (useful if it changes)
	 */
	setPlatform(platform: Platform) {
		this.platform = platform;
	}

	/**
	 * Get current platform
	 */
	getPlatform(): Platform {
		return this.platform;
	}

	/**
	 * Cleanup and remove event listeners
	 */
	destroy() {
		if (typeof window !== 'undefined' && this.initialized) {
			window.removeEventListener('keydown', this.handleKeyDown);
		}
		this.registeredKeybinds.clear();
		this.activeScopes.clear();
		this.activeScopes.add('global');
		this.initialized = false;
	}

	/**
	 * Get count of registered keybinds (for debugging)
	 */
	getRegisteredCount(): number {
		return this.registeredKeybinds.size;
	}

	/**
	 * Get all registered keybind IDs (for debugging)
	 */
	getRegisteredIds(): string[] {
		return Array.from(this.registeredKeybinds.keys());
	}
}

// Global singleton instance
let listenerInstance: WebKeybindListener | null = null;

/**
 * Get the singleton WebKeybindListener instance
 */
export function getWebListener(): WebKeybindListener {
	if (!listenerInstance) {
		listenerInstance = new WebKeybindListener();
	}
	return listenerInstance;
}

/**
 * Reset the singleton (useful for testing)
 */
export function resetWebListener(): void {
	if (listenerInstance) {
		listenerInstance.destroy();
		listenerInstance = null;
	}
}
