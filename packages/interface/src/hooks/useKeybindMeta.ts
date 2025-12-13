/**
 * useKeybindMeta Hook
 *
 * Get metadata about a keybind (display string, label, etc.)
 * Useful for displaying keybind hints in UI.
 */

import { useMemo } from 'react';
import type { KeyCombo, KeybindScope } from '../util/keybinds/types';
import type { KeybindId } from '../util/keybinds/registry';
import { getKeybind } from '../util/keybinds/registry';
import { getComboForPlatform, getCurrentPlatform, toDisplayString } from '../util/keybinds/platform';

export interface KeybindMeta {
	/** The keybind ID */
	id: string;
	/** Human-readable label */
	label: string;
	/** Platform-appropriate display string (e.g., "Cmd+C" or "Ctrl+C") */
	displayString: string;
	/** The key combo for current platform */
	combo: KeyCombo;
	/** The keybind's scope */
	scope: KeybindScope;
}

/**
 * Get metadata about a keybind for display purposes
 *
 * @param keybindId - Type-safe keybind ID from registry
 * @returns KeybindMeta object or null if keybind not found
 *
 * @example
 * ```tsx
 * function CopyButton() {
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
			scope: keybind.scope
		};
	}, [keybindId]);
}

/**
 * Get display string for a keybind ID (convenience function)
 *
 * @param keybindId - Type-safe keybind ID from registry
 * @returns Display string or empty string if not found
 */
export function useKeybindDisplayString(keybindId: KeybindId): string {
	const meta = useKeybindMeta(keybindId);
	return meta?.displayString ?? '';
}

export default useKeybindMeta;
