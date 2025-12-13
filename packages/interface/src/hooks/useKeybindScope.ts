/**
 * useKeybindScope Hook
 *
 * Activates a keybind scope for the lifetime of the component.
 * This allows component-specific keybinds to be active only when
 * the component is mounted.
 */

import { useEffect } from 'react';
import type { KeybindScope } from '../util/keybinds/types';
import { getWebListener } from '../util/keybinds/listener';
import { usePlatform } from '../platform';

/**
 * Activate a keybind scope while the component is mounted
 *
 * @param scope - The scope to activate ('explorer', 'mediaViewer', etc.)
 *
 * @example
 * ```tsx
 * function ExplorerView() {
 *   // Activate explorer scope - keybinds with scope='explorer' will now work
 *   useKeybindScope('explorer');
 *
 *   return <div>...</div>;
 * }
 * ```
 */
export function useKeybindScope(scope: KeybindScope) {
	const platform = usePlatform();

	useEffect(() => {
		// Only affects web listener
		// Tauri global shortcuts don't use scopes (they're always active)
		// For Tauri, scope management would need to be handled differently
		// (e.g., by conditionally registering/unregistering shortcuts)

		const listener = getWebListener();
		listener.pushScope(scope);

		return () => {
			listener.popScope(scope);
		};
	}, [scope, platform]);
}

export default useKeybindScope;
