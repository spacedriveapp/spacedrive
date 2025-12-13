import { useEffect } from 'react';
import type { KeybindScope } from '../util/keybinds/types';
import { getWebListener } from '../util/keybinds/listener';
import { usePlatform } from '../platform';

/**
 * Push a keybind scope for the lifetime of the component
 *
 * This allows component-specific keybinds to be active only when
 * the component is mounted. Scopes work in a stack-like fashion:
 * - 'global' scope is always active
 * - Other scopes are active only when explicitly pushed
 *
 * Note: On Tauri with global shortcuts, scopes are managed differently
 * since shortcuts are OS-level. This hook primarily affects the web listener.
 *
 * @param scope - The keybind scope to activate
 *
 * @example
 * ```tsx
 * function ExplorerView() {
 *   // Activate explorer scope while this component is mounted
 *   useKeybindScope('explorer');
 *
 *   // Now explorer keybinds like 'explorer.copy' will be active
 *   useKeybind('explorer.copy', () => copyFiles());
 *
 *   return <div>Explorer content</div>;
 * }
 * ```
 */
export function useKeybindScope(scope: KeybindScope) {
	const platform = usePlatform();

	useEffect(() => {
		// Scopes primarily affect the web listener
		// Tauri global shortcuts don't have the same scope concept
		// since they're registered at the OS level
		if (platform.platform !== 'tauri') {
			const listener = getWebListener();
			listener.pushScope(scope);

			return () => {
				listener.popScope(scope);
			};
		}

		// For Tauri, we still push the scope to the web listener
		// in case it falls back to web mode
		const listener = getWebListener();
		listener.pushScope(scope);

		return () => {
			listener.popScope(scope);
		};
	}, [scope, platform.platform]);
}
