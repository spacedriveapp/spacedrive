import type { KeyCombo, Platform } from './types';
import { getCurrentPlatform, matchesKeyCombo } from './platform';

export type KeybindHandler = () => void | Promise<void>;

interface RegisteredKeybind {
	combo: KeyCombo;
	handler: KeybindHandler;
	scope: string;
	preventDefault: boolean;
}

// Singleton listener for web platform
class WebKeybindListener {
	private registeredKeybinds = new Map<string, RegisteredKeybind>();
	private activeScopes = new Set<string>(['global']);
	private platform: Platform;

	constructor() {
		this.platform = getCurrentPlatform();

		if (typeof window !== 'undefined') {
			window.addEventListener('keydown', this.handleKeyDown);
		}
	}

	register(id: string, combo: KeyCombo, handler: KeybindHandler, scope: string, preventDefault = false) {
		this.registeredKeybinds.set(id, {
			combo,
			handler,
			scope,
			preventDefault
		});
	}

	unregister(id: string) {
		this.registeredKeybinds.delete(id);
	}

	pushScope(scope: string) {
		this.activeScopes.add(scope);
	}

	popScope(scope: string) {
		this.activeScopes.delete(scope);
	}

	isRegistered(id: string): boolean {
		return this.registeredKeybinds.has(id);
	}

	private handleKeyDown = async (e: KeyboardEvent) => {
		// Skip if user is typing in an input or textarea (unless explicitly handled)
		const target = e.target as HTMLElement;
		const tagName = target.tagName.toLowerCase();
		const isEditable = target.isContentEditable;
		const isInput = tagName === 'input' || tagName === 'textarea' || tagName === 'select' || isEditable;

		// Find matching keybind
		for (const [_id, keybind] of this.registeredKeybinds) {
			// Check if scope is active
			if (!this.isScopeActive(keybind.scope)) continue;

			// Check if key combo matches
			if (matchesKeyCombo(e, keybind.combo, this.platform)) {
				// For keybinds without modifiers, skip if we're in an input
				// (unless it's Escape which should always work)
				if (isInput && keybind.combo.modifiers.length === 0 && keybind.combo.key !== 'Escape') {
					continue;
				}

				if (keybind.preventDefault) {
					e.preventDefault();
					e.stopPropagation();
				}

				try {
					await keybind.handler();
				} catch (error) {
					console.error(`Keybind handler error:`, error);
				}
				return;
			}
		}
	};

	private isScopeActive(scope: string): boolean {
		return scope === 'global' || this.activeScopes.has(scope);
	}

	destroy() {
		if (typeof window !== 'undefined') {
			window.removeEventListener('keydown', this.handleKeyDown);
		}
		this.registeredKeybinds.clear();
	}
}

// Global singleton instance
let listenerInstance: WebKeybindListener | null = null;

export function getWebListener(): WebKeybindListener {
	if (!listenerInstance) {
		listenerInstance = new WebKeybindListener();
	}
	return listenerInstance;
}

// For testing purposes
export function resetWebListener(): void {
	if (listenerInstance) {
		listenerInstance.destroy();
		listenerInstance = null;
	}
}
