import { useMemo } from 'react';
import type { KeybindId } from '../util/keybinds/registry';
import { getKeybind } from '../util/keybinds/registry';
import { getComboForPlatform, getCurrentPlatform, toDisplayString } from '../util/keybinds/platform';

/**
 * Get metadata about a keybind (display string, label, etc.)
 *
 * @param keybindId - Type-safe keybind ID from registry
 * @returns Object with keybind metadata, or null if not found
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
 * Get the display string for a keybind ID
 *
 * Convenience function for when you only need the display string
 *
 * @param keybindId - Type-safe keybind ID from registry
 * @returns Display string (e.g., "Cmd+C" or "Ctrl+C"), or empty string if not found
 */
export function getKeybindDisplayString(keybindId: KeybindId): string {
	const keybind = getKeybind(keybindId);
	if (!keybind) return '';

	const platform = getCurrentPlatform();
	const combo = getComboForPlatform(keybind.combo, platform);
	return toDisplayString(combo, platform);
}
