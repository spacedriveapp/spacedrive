/**
 * useKeybindScope Hook
 *
 * Manages keybind scope activation for components.
 * When a component using this hook is mounted, its scope becomes active
 * and scope-specific keybinds will respond to keyboard events.
 */

import { useEffect } from 'react';
import type { KeybindScope } from '../util/keybinds/types';
import { getWebListener } from '../util/keybinds/listener';
import { usePlatform } from '../platform';

/**
 * Push a keybind scope for the lifetime of the component.
 *
 * This allows component-specific keybinds to be active only when
 * the component is mounted. When the component unmounts, the scope
 * is automatically deactivated.
 *
 * Note: On Tauri, scoping is handled differently (all shortcuts are global),
 * so this hook only affects web platform behavior.
 *
 * @param scope - The keybind scope to activate
 *
 * @example
 * ```tsx
 * function ExplorerView() {
 *   // Activate explorer scope while this component is mounted
 *   useKeybindScope('explorer');
 *
 *   // Now explorer-scoped keybinds will be active
 *   useKeybind('explorer.copy', handleCopy);
 *
 *   return <div>...</div>;
 * }
 * ```
 */
export function useKeybindScope(scope: KeybindScope): void {
	const platform = usePlatform();

	useEffect(() => {
		// Only affects web listener (Tauri uses global shortcuts)
		if (platform.platform === 'web') {
			const listener = getWebListener();
			listener.pushScope(scope);

			return () => {
				listener.popScope(scope);
			};
		}
	}, [scope, platform.platform]);
}
