import { useMemo } from 'react';
import type { KeybindId } from '../util/keybinds/registry';
import { getKeybind } from '../util/keybinds/registry';
import type { KeyCombo, KeybindScope } from '../util/keybinds/types';
import { getComboForPlatform, getCurrentPlatform, toDisplayString } from '../util/keybinds/platform';

export interface KeybindMeta {
	/** The keybind ID */
	id: string;
	/** Human-readable label for the keybind */
	label: string;
	/** Platform-specific display string (e.g., "⌘C" on macOS, "Ctrl+C" on Windows) */
	displayString: string;
	/** The resolved key combo for the current platform */
	combo: KeyCombo;
	/** The scope this keybind belongs to */
	scope: KeybindScope;
}

/**
 * Get metadata about a keybind (display string, label, etc.)
 *
 * @param keybindId - Type-safe keybind ID from registry
 * @returns Keybind metadata or null if not found
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
			scope: keybind.scope,
		};
	}, [keybindId]);
}

/**
 * Get display string for a keybind without the full metadata.
 * Convenience hook for when you only need the display string.
 *
 * @param keybindId - Type-safe keybind ID from registry
 * @returns Display string or empty string if not found
 *
 * @example
 * ```tsx
 * function MenuItem() {
 *   const shortcut = useKeybindDisplayString('explorer.copy');
 *   return <span>{shortcut}</span>; // "⌘C" on macOS
 * }
 * ```
 */
export function useKeybindDisplayString(keybindId: KeybindId): string {
	const meta = useKeybindMeta(keybindId);
	return meta?.displayString ?? '';
}
