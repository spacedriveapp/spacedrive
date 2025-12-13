import { useMemo } from 'react';
import type { KeybindId } from '../util/keybinds/registry';
import { getKeybind } from '../util/keybinds/registry';
import {
	getComboForPlatform,
	getCurrentPlatform,
	toDisplayString
} from '../util/keybinds/platform';

/**
 * Get metadata about a keybind (display string, label, etc.)
 *
 * This hook is useful for displaying keybind shortcuts in the UI,
 * such as in context menus or tooltips.
 *
 * @param keybindId - Type-safe keybind ID from registry
 * @returns Keybind metadata or null if not found
 *
 * @example
 * ```tsx
 * function MenuItem({ action }: { action: KeybindId }) {
 *   const meta = useKeybindMeta(action);
 *
 *   return (
 *     <button>
 *       {meta?.label}
 *       {meta && <span className="keybind">{meta.displayString}</span>}
 *     </button>
 *   );
 * }
 * ```
 */
export function useKeybindMeta(keybindId: KeybindId) {
	return useMemo(() => {
		const keybind = getKeybind(keybindId);
		if (!keybind) return null;

		const platform = getCurrentPlatform();
		const combo = getComboForPlatform(keybind.combo, platform);
		const displayString = toDisplayString(combo, platform);

		return {
			id: keybind.id,
			label: keybind.label,
			displayString,
			combo,
			scope: keybind.scope
		};
	}, [keybindId]);
}

/**
 * Get display string for a keybind
 *
 * Convenience function that returns just the display string for a keybind.
 * Useful when you only need the shortcut text.
 *
 * @param keybindId - Type-safe keybind ID from registry
 * @returns Display string (e.g., "⌘C" on macOS, "Ctrl+C" on Windows) or undefined
 *
 * @example
 * ```tsx
 * const copyShortcut = useKeybindDisplayString('explorer.copy');
 * // Returns "⌘C" on macOS, "Ctrl+C" on Windows/Linux
 * ```
 */
export function useKeybindDisplayString(keybindId: KeybindId): string | undefined {
	const meta = useKeybindMeta(keybindId);
	return meta?.displayString;
}
