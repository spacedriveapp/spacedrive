/**
 * useKeybindMeta Hook
 *
 * Provides metadata about a keybind, including its display string.
 * Useful for showing keybind hints in context menus and tooltips.
 */

import { useMemo } from 'react';
import type { KeybindId } from '../util/keybinds/registry';
import { getKeybind } from '../util/keybinds/registry';
import { getComboForPlatform, getCurrentPlatform, toDisplayString } from '../util/keybinds/platform';
import type { KeyCombo, KeybindScope } from '../util/keybinds/types';

export interface KeybindMeta {
	/** The keybind ID */
	id: string;
	/** Human-readable label for the keybind */
	label: string;
	/** Platform-specific display string (e.g., "Cmd+C" or "Ctrl+C") */
	displayString: string;
	/** The resolved key combo for the current platform */
	combo: KeyCombo;
	/** The keybind's scope */
	scope: KeybindScope;
}

/**
 * Get metadata about a keybind, including the platform-specific display string.
 *
 * This hook is useful for:
 * - Displaying keybind hints in context menus
 * - Showing keyboard shortcuts in tooltips
 * - Building keyboard shortcut reference documentation
 *
 * @param keybindId - Type-safe keybind ID from the registry
 * @returns Keybind metadata or null if the keybind is not found
 *
 * @example
 * ```tsx
 * function ContextMenuButton() {
 *   const meta = useKeybindMeta('explorer.copy');
 *
 *   return (
 *     <button title={`Copy (${meta?.displayString})`}>
 *       Copy
 *     </button>
 *   );
 * }
 * ```
 */
export function useKeybindMeta(keybindId: KeybindId): KeybindMeta | null {
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
			scope: keybind.scope,
		};
	}, [keybindId]);
}

/**
 * Get the display string for a keybind directly (non-hook version).
 * Useful for places where hooks cannot be used.
 *
 * @param keybindId - Type-safe keybind ID from the registry
 * @returns Platform-specific display string or undefined if not found
 */
export function getKeybindDisplayString(keybindId: KeybindId): string | undefined {
	const keybind = getKeybind(keybindId);
	if (!keybind) return undefined;

	const platform = getCurrentPlatform();
	const combo = getComboForPlatform(keybind.combo, platform);
	return toDisplayString(combo, platform);
}
