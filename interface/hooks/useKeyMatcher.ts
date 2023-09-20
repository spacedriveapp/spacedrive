import { ModifierKeys, modifierSymbols } from "@sd/ui";
import { OperatingSystem } from "..";
import { useOperatingSystem } from "./useOperatingSystem";

type keysToMatch = 'Meta' | 'Alt'
type keysOsMap = Record<keysToMatch,osKeys>
type osKeys = Record<OperatingSystem, {key: Partial<keyof typeof ModifierKeys>, icon: string}>

//This is a helper function to handle the possibility of a modifier key being undefined due to OS initial check
const modifierKey = (key: keyof typeof ModifierKeys, os: 'Windows' | 'macOS' | 'Other') => {
	return modifierSymbols[key][os] ?? modifierSymbols[key]['Other'];
}

//Match macOS keys to Windows keys and others
const keysOsMap: keysOsMap = {
	'Meta': {
		'macOS': {key: 'Meta', icon: modifierKey(ModifierKeys.Meta, 'macOS') },
		'windows': {key: 'Control', icon: modifierKey(ModifierKeys.Control, 'Windows') },
		'browser': {key: 'Control', icon: modifierKey(ModifierKeys.Control, 'Windows') },
		'linux': {key: 'Control', icon: modifierKey(ModifierKeys.Control, 'Windows') },
		'unknown': {key: 'Control', icon: modifierKey(ModifierKeys.Control, 'Windows') },
	},
	'Alt': {
		'macOS': {key: 'Alt', icon: modifierKey(ModifierKeys.Alt, 'macOS') },
		'windows': {key: 'Alt', icon: modifierKey(ModifierKeys.Alt, 'Other') },
		'browser': {key: 'Alt', icon: modifierKey(ModifierKeys.Alt, 'Other') },
		'linux': {key: 'Alt', icon: modifierKey(ModifierKeys.Alt, 'Other') },
		'unknown': {key: 'Alt', icon: modifierKey(ModifierKeys.Alt, 'Other') },
	},
} as const

export function useKeyMatcher(arg: keyof typeof keysOsMap): osKeys[OperatingSystem] {
	const os = useOperatingSystem();
	const key = keysOsMap[arg][os];
	return key;
}
