import { useEffect } from 'react';
import type { KeybindScope } from '../util/keybinds/types';
import { getWebListener } from '../util/keybinds/listener';
import { usePlatform } from '../platform';

/**
 * Push a keybind scope for the lifetime of the component
 *
 * This allows component-specific keybinds to be active only when
 * the component is mounted.
 *
 * @example
 * ```tsx
 * function ExplorerView() {
 *   // Activate explorer scope while this component is mounted
 *   useKeybindScope('explorer');
 *
 *   return <div>...</div>;
 * }
 * ```
 */
export function useKeybindScope(scope: KeybindScope) {
	const platform = usePlatform();

	useEffect(() => {
		// Web listener handles scope management
		// Note: In future, Tauri can use a similar mechanism if needed
		const listener = getWebListener();
		listener.pushScope(scope);

		return () => {
			listener.popScope(scope);
		};
	}, [scope, platform]);
}
