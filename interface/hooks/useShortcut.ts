import { useSnapshot } from 'valtio';
import { valtioPersist } from '@sd/client';

import { OperatingSystem } from '../util/Platform';
import { useOperatingSystem } from './useOperatingSystem';

const state = {
	gridView: {
		keys: {
			macOS: ['Meta', '1'],
			all: ['Control', '1']
		}
	},
	listView: {
		keys: {
			macOS: ['Meta', '2'],
			all: ['Control', '2']
		}
	},
	mediaView: {
		keys: {
			macOS: ['Meta', '3'],
			all: ['Control', '3']
		}
	},
	showHiddenFiles: {
		keys: {
			macOS: ['Meta', 'Shift', '.'],
			all: ['Control', 'Shift', '.']
		}
	},
	showPathBar: {
		keys: {
			macOS: ['Meta', 'KeyP'],
			all: ['Control', 'KeyP']
		}
	},
	showInspector: {
		keys: {
			macOS: ['Meta', 'KeyI'],
			all: ['Control', 'KeyI']
		}
	},
	toggleJobManager: {
		keys: {
			macOS: ['Meta', 'KeyJ'],
			all: ['Control', 'KeyJ']
		}
	},
	toggleQuickPreview: {
		keys: {
			all: ['space']
		}
	},
	toggleMetaData: {
		keys: {
			macOS: ['Meta', 'KeyI'],
			all: ['Control', 'KeyI']
		}
	},
	revealNative: {
		keys: {
			macOS: ['Meta', 'KeyY'],
			all: ['Control', 'KeyY']
		}
	},
	renameObject: {
		keys: {
			macOS: ['Enter'],
			all: ['F2']
		}
	},
	openItem: {
		keys: {
			macOS: ['Meta', 'KeyO'],
			all: ['Enter']
		}
	},
	delItem: {
		keys: {
			macOS: ['Meta', 'Backspace'],
			all: ['Delete']
		}
	},
	navBackwardHistory: {
		keys: {
			macOS: ['Meta', '['],
			all: ['Control', '[']
		}
	},
	navForwardHistory: {
		keys: {
			macOS: ['Meta', ']'],
			all: ['Control', ']']
		}
	},
	navToSettings: {
		keys: {
			macOS: ['Shift', 'Meta', 'KeyT'],
			all: ['Shift', 'Control', 'KeyT']
		}
	},
	navToOverview: {
		keys: {
			macOS: ['Shift', 'Meta', 'KeyO'],
			all: ['Shift', 'Control', 'KeyO']
		}
	},
	navExpObjects: {
		keys: {
			all: ['Control', 'ArrowRight']
		}
	}
} satisfies Record<
	string,
	{
		keys: {
			[os in OperatingSystem | 'all']?: string[];
		};
	}
>;

const shortcutsStore = valtioPersist('sd-shortcuts', {
	...state
});

export function useShortcutsStore() {
	return useSnapshot(shortcutsStore);
}

export function getShortcutsStore() {
	return shortcutsStore;
}

type keyofState = keyof typeof state;

//returns an object with the shortcuts for the current OS
//so we don't have to handle this in the components
export const useShortcut = () => {
	const os = useOperatingSystem();
	const shortcutsStore = getShortcutsStore();
	const shortcutsForOs = {} as Record<keyofState, string[]>;

	for (const shortcut in shortcutsStore) {
		const shortcutKeys = shortcutsStore[shortcut as keyofState].keys;

		for (const keys in shortcutKeys) {
			if (keys === os || keys === 'all') {
				shortcutsForOs[shortcut as keyofState] =
					shortcutKeys[keys as keyof typeof shortcutKeys];
			}
		}
	}
	return shortcutsForOs;
};
