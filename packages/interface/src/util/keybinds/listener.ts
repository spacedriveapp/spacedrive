import type { KeyCombo, Platform } from './types';
import { getCurrentPlatform, normalizeModifiers } from './platform';

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

	register(
		id: string,
		combo: KeyCombo,
		handler: KeybindHandler,
		scope: string,
		preventDefault = false
	) {
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

	private handleKeyDown = async (e: KeyboardEvent) => {
		// Skip if target is an input element (unless modifier keys are pressed)
		const target = e.target as HTMLElement;
		const isInputElement =
			target.tagName === 'INPUT' ||
			target.tagName === 'TEXTAREA' ||
			target.isContentEditable;

		// Find matching keybind
		for (const [, keybind] of this.registeredKeybinds) {
			// Check if scope is active
			if (!this.isScopeActive(keybind.scope)) continue;

			// Check if key combo matches
			if (this.matchesCombo(e, keybind.combo)) {
				// For input elements, only trigger keybinds with modifiers
				if (isInputElement && keybind.combo.modifiers.length === 0) {
					continue;
				}

				if (keybind.preventDefault) {
					e.preventDefault();
					e.stopPropagation();
				}

				await keybind.handler();
				return;
			}
		}
	};

	private isScopeActive(scope: string): boolean {
		return scope === 'global' || this.activeScopes.has(scope);
	}

	private matchesCombo(e: KeyboardEvent, combo: KeyCombo): boolean {
		const normalizedModifiers = normalizeModifiers(combo.modifiers, this.platform);

		// Check modifiers
		for (const modifier of normalizedModifiers) {
			switch (modifier) {
				case 'Cmd':
					// On macOS, Cmd = metaKey; on Windows/Linux, Cmd normalizes to Ctrl
					if (this.platform === 'macos') {
						if (!e.metaKey) return false;
					} else {
						if (!e.ctrlKey) return false;
					}
					break;
				case 'Ctrl':
					if (!e.ctrlKey) return false;
					break;
				case 'Alt':
					if (!e.altKey) return false;
					break;
				case 'Shift':
					if (!e.shiftKey) return false;
					break;
			}
		}

		// Check that no extra modifiers are pressed (unless they're in the combo)
		const hasCmd = normalizedModifiers.includes('Cmd');
		const hasCtrl = normalizedModifiers.includes('Ctrl');
		const hasAlt = normalizedModifiers.includes('Alt');
		const hasShift = normalizedModifiers.includes('Shift');

		// On macOS, Cmd is metaKey; on other platforms, Cmd normalizes to Ctrl
		if (this.platform === 'macos') {
			if (!hasCmd && e.metaKey) return false;
			if (!hasCtrl && e.ctrlKey) return false;
		} else {
			// On non-macOS, both Cmd and Ctrl map to ctrlKey, so we need to be careful
			if (!hasCmd && !hasCtrl && e.ctrlKey) return false;
			if (e.metaKey) return false; // Windows key shouldn't trigger
		}

		if (!hasAlt && e.altKey) return false;
		if (!hasShift && e.shiftKey) return false;

		// Check key - normalize for comparison
		const eventKey = e.key.toLowerCase();
		const comboKey = combo.key.toLowerCase();

		// Handle special key mappings
		if (comboKey === 'space' && eventKey === ' ') return true;
		if (comboKey === 'arrowup' && eventKey === 'arrowup') return true;
		if (comboKey === 'arrowdown' && eventKey === 'arrowdown') return true;
		if (comboKey === 'arrowleft' && eventKey === 'arrowleft') return true;
		if (comboKey === 'arrowright' && eventKey === 'arrowright') return true;

		return eventKey === comboKey;
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

// For testing or cleanup purposes
export function destroyWebListener(): void {
	if (listenerInstance) {
		listenerInstance.destroy();
		listenerInstance = null;
	}
}
