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
			macOS: ['Alt', 'Meta', 'KeyP'],
			all: ['Alt', 'Control', 'KeyP']
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
	quickPreviewMoveBetweenItems: {
		keys: {
			all: ['ArrowLeft', 'ArrowRight']
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
	rescan: {
		keys: {
			macOS: ['Meta', 'KeyR'],
			all: ['Control', 'KeyR']
		}
	},
	cutObject: {
		keys: {
			macOS: ['Meta', 'KeyX'],
			all: ['Control', 'KeyX']
		}
	},
	copyObject: {
		keys: {
			macOS: ['Meta', 'KeyC'],
			all: ['Control', 'KeyC']
		}
	},
	pasteObject: {
		keys: {
			macOS: ['Meta', 'KeyV'],
			all: ['Control', 'KeyV']
		}
	},
	duplicateObject: {
		keys: {
			macOS: ['Meta', 'KeyD'],
			all: ['Control', 'KeyD']
		}
	},
	openObject: {
		keys: {
			macOS: ['Meta', 'KeyO'],
			all: ['Enter']
		}
	},
	quickPreviewOpenNative: {
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
	gridObjectsNav: {
		keys: {
			all: ['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'Escape']
		}
	},
	listObjectsNav: {
		keys: {
			all: ['ArrowUp', 'ArrowDown', 'Escape']
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

const shortcutsStore = valtioPersist('sd-shortcuts', state);

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

	//loop through the shortcuts and return the correct one for the current OS
	//otherwise return 'all'
	for (const shortcut in shortcutsStore) {
		const osKeys = shortcutsStore[shortcut as keyofState].keys;
		shortcutsForOs[shortcut as keyofState] = osKeys[os as keyof typeof osKeys] || osKeys['all'];
	}
	return shortcutsForOs;
};
