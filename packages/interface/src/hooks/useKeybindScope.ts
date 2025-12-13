import { useEffect } from 'react';
import type { KeybindScope } from '../util/keybinds/types';
import { getWebListener } from '../util/keybinds/listener';

/**
 * Push a keybind scope for the lifetime of the component.
 *
 * This allows component-specific keybinds to be active only when
 * the component is mounted. Keybinds with the specified scope will
 * only trigger when this scope is active.
 *
 * The 'global' scope is always active and doesn't need to be pushed.
 *
 * @param scope - The scope to activate
 *
 * @example
 * ```tsx
 * function ExplorerView() {
 *   // Activate explorer scope when this component mounts
 *   useKeybindScope('explorer');
 *
 *   return <div>...</div>;
 * }
 * ```
 */
export function useKeybindScope(scope: KeybindScope): void {
	useEffect(() => {
		// Don't push global scope - it's always active
		if (scope === 'global') return;

		const listener = getWebListener();
		listener.pushScope(scope);

		return () => {
			listener.popScope(scope);
		};
	}, [scope]);
}

/**
 * Check if a scope is currently active.
 *
 * @param scope - The scope to check
 * @returns true if the scope is active
 */
export function isScopeActive(scope: KeybindScope): boolean {
	if (scope === 'global') return true;
	return getWebListener().hasScope(scope);
}
