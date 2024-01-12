import { valtioPersist } from '@sd/client';
import { modifierSymbols } from '@sd/ui';
import { useKeys } from 'rooks';
import { useSnapshot } from 'valtio';
import { useRoutingContext } from '~/RoutingContext';
import { OperatingSystem } from '~/util/Platform';

import { useOperatingSystem } from './useOperatingSystem';

//This will be refactored in the near future
//as we adopt different shortcuts for different platforms
//aswell. i.e Mobile.

type Shortcut = {
	action: string;
	keys: {
		[K in OperatingSystem | 'all']?: string[];
	};
	icons: {
		[K in OperatingSystem | 'all']?: string[];
	};
};

type ShortcutCategory = {
	description: string;
  } & Record<string, any> //TODO: fix types

export type TShortcutState = {
	shortcuts: Record<'Dialogs' | 'Pages' | 'Explorer', ShortcutCategory>;
  };

export const ShortcutState: TShortcutState = {
	shortcuts: {
		Dialogs: {
			description: 'To perform actions and operations',
			toggleJobManager: {
				action: 'Toggle job manager',
				keys: {
					macOS: ['Meta', 'KeyJ'],
					all: ['Control', 'KeyJ']
				},
				icons: {
					macOS: [modifierSymbols.Meta.macOS as string, 'J'],
					all: [modifierSymbols.Control.Other, 'J']
				}
			}
		},
		Pages: {
			description: 'Different pages in the app',
			navBackwardHistory: {
				action: 'Navigate backwards',
				keys: {
					macOS: ['Meta', '['],
					all: ['Control', '[']
				},
				icons: {
					macOS: [modifierSymbols.Meta.macOS as string, '['],
					all: [modifierSymbols.Control.Other, '[']
				}
			},
			navForwardHistory: {
				action: 'Navigate forwards',
				keys: {
					macOS: ['Meta', ']'],
					all: ['Control', ']']
				},
				icons: {
					macOS: [modifierSymbols.Meta.macOS as string, ']'],
					all: [modifierSymbols.Control.Other, ']']
				}
			},
			navToSettings: {
				action: 'Navigate to Settings page',
				keys: {
					macOS: ['Shift', 'Meta', 'KeyT'],
					all: ['Shift', 'Control', 'KeyT']
				},
				icons: {
					macOS: [
						modifierSymbols.Shift.macOS as string,
						modifierSymbols.Meta.macOS as string,
						'T'
					],
					all: [modifierSymbols.Shift.Other, modifierSymbols.Control.Other, 'T']
				}
			}
		},
		Explorer: {
			description: 'To navigate and interact with the file system',
			gridView: {
				action: 'Switch to grid view',
				keys: {
					macOS: ['Meta', '1'],
					all: ['Control', '1']
				},
				icons: {
					macOS: [modifierSymbols.Meta.macOS as string, '1'],
					all: [modifierSymbols.Control.Other, '1']
				}
			},
			listView: {
				action: 'Switch to list view',
				keys: {
					macOS: ['Meta', '2'],
					all: ['Control', '2']
				},
				icons: {
					macOS: [modifierSymbols.Meta.macOS as string, '2'],
					all: [modifierSymbols.Control.Other, '2']
				}
			},
			mediaView: {
				action: 'Switch to media view',
				keys: {
					macOS: ['Meta', '3'],
					all: ['Control', '3']
				},
				icons: {
					macOS: [modifierSymbols.Meta.macOS as string, '3'],
					all: [modifierSymbols.Control.Other, '3']
				}
			},
			showHiddenFiles: {
				action: 'Toggle hidden files',
				keys: {
					macOS: ['Meta', 'Shift', '.'],
					all: ['Control', 'KeyH']
				},
				icons: {
					macOS: [
						modifierSymbols.Meta.macOS as string,
						modifierSymbols.Shift.macOS as string,
						'.'
					],
					all: [modifierSymbols.Control.Other, 'h']
				}
			},
			showPathBar: {
				action: 'Toggle path bar',
				keys: {
					macOS: ['Alt', 'Meta', 'KeyP'],
					all: ['Alt', 'Control', 'KeyP']
				},
				icons: {
					macOS: [
						modifierSymbols.Alt.macOS as string,
						modifierSymbols.Meta.macOS as string,
						'p'
					],
					all: [modifierSymbols.Alt.Other, modifierSymbols.Control.Other, 'p']
				}
			},
			showImageSlider: {
				action: 'Toggle image slider within quick preview',
				keys: {
					macOS: ['Alt', 'Meta', 'KeyM'],
					all: ['Alt', 'Control', 'KeyM']
				},
				icons: {
					macOS: [
						modifierSymbols.Alt.macOS as string,
						modifierSymbols.Meta.macOS as string,
						'm'
					],
					all: [modifierSymbols.Alt.Other, modifierSymbols.Control.Other, 'm']
				}
			},
			showInspector: {
				action: 'Toggle inspector',
				keys: {
					macOS: ['Meta', 'KeyI'],
					all: ['Control', 'KeyI']
				},
				icons: {
					macOS: [modifierSymbols.Meta.macOS as string, 'i'],
					all: [modifierSymbols.Control.Other, 'i']
				}
			},
			toggleQuickPreview: {
				action: 'Toggle quick preview',
				keys: {
					all: [' ']
				},
				icons: {
					all: [' ']
				}
			},
			toggleMetaData: {
				action: 'Toggle metadata',
				keys: {
					macOS: ['Meta', 'KeyI'],
					all: ['Control', 'KeyI']
				},
				icons: {
					macOS: [modifierSymbols.Meta.macOS as string, 'I'],
					all: [modifierSymbols.Control.Other, 'I']
				}
			},
			quickPreviewMoveBack: {
				action: 'Move back within quick preview',
				keys: {
					all: ['ArrowLeft']
				},
				icons: {
					all: ['ArrowLeft']
				}
			},
			quickPreviewMoveForward: {
				action: 'Move forward within quick preview',
				keys: {
					all: ['ArrowRight']
				},
				icons: {
					all: ['ArrowRight']
				}
			},
			revealNative: {
				action: 'Reveal in native file manager',
				keys: {
					macOS: ['Meta', 'KeyY'],
					all: ['Control', 'KeyY']
				},
				icons: {
					macOS: [modifierSymbols.Meta.macOS as string, 'Y'],
					all: [modifierSymbols.Control.Other, 'Y']
				}
			},
			renameObject: {
				action: 'Rename object',
				keys: {
					macOS: ['Enter'],
					all: ['F2']
				},
				icons: {
					windows: ['F2'],
					macOS: ['Enter']
				}
			},
			rescan: {
				action: 'Rescan location',
				keys: {
					macOS: ['Meta', 'KeyR'],
					all: ['Control', 'KeyR']
				},
				icons: {
					macOS: [modifierSymbols.Meta.macOS as string, 'R'],
					all: [modifierSymbols.Control.Other, 'R']
				}
			},
			cutObject: {
				action: 'Cut object',
				keys: {
					macOS: ['Meta', 'KeyX'],
					all: ['Control', 'KeyX']
				},
				icons: {
					macOS: [modifierSymbols.Meta.macOS as string, 'X'],
					all: [modifierSymbols.Control.Other, 'X']
				}
			},
			copyObject: {
				action: 'Copy object',
				keys: {
					macOS: ['Meta', 'KeyC'],
					all: ['Control', 'KeyC']
				},
				icons: {
					macOS: [modifierSymbols.Meta.macOS as string, 'C'],
					all: [modifierSymbols.Control.Other, 'C']
				}
			},
			pasteObject: {
				action: 'Paste object',
				keys: {
					macOS: ['Meta', 'KeyV'],
					all: ['Control', 'KeyV']
				},
				icons: {
					macOS: [modifierSymbols.Meta.macOS as string, 'V'],
					all: [modifierSymbols.Control.Other, 'V']
				}
			},
			duplicateObject: {
				action: 'Duplicate object',
				keys: {
					macOS: ['Meta', 'KeyD'],
					all: ['Control', 'KeyD']
				},
				icons: {
					macOS: [modifierSymbols.Meta.macOS as string, 'D'],
					all: [modifierSymbols.Control.Other, 'D']
				}
			},
			openObject: {
				action: 'Open object',
				keys: {
					macOS: ['Meta', 'KeyO'],
					all: ['Enter']
				},
				icons: {
					macOS: [modifierSymbols.Meta.macOS as string, 'O'],
					all: ['Enter']
				}
			},
			quickPreviewOpenNative: {
				action: 'Open object from quick preview in native file manager',
				keys: {
					macOS: ['Meta', 'KeyO'],
					all: ['Enter']
				},
				icons: {
					macOS: [modifierSymbols.Meta.macOS as string, 'O'],
					all: ['Enter']
				}
			},
			delItem: {
				action: 'Delete object',
				keys: {
					macOS: ['Meta', 'Backspace'],
					all: ['Delete']
				},
				icons: {
					macOS: [modifierSymbols.Meta.macOS as string, 'Backspace'],
					all: ['Delete']
				}
			},
			explorerEscape: {
				action: 'Cancel selection',
				keys: {
					all: ['Escape']
				},
				icons: {
					all: ['Escape']
				}
			},
			explorerDown: {
				action: 'Navigate files downwards',
				keys: {
					all: ['ArrowDown']
				},
				icons: {
					all: ['ArrowDown']
				}
			},
			explorerUp: {
				action: 'Navigate files upwards',
				keys: {
					all: ['ArrowUp']
				},
				icons: {
					all: ['ArrowUp']
				}
			},
			explorerLeft: {
				action: 'Navigate files leftwards',
				keys: {
					all: ['ArrowLeft']
				},
				icons: {
					all: ['ArrowLeft']
				}
			},
			explorerRight: {
				action: 'Navigate files rightwards',
				keys: {
					all: ['ArrowRight']
				},
				icons: {
					all: ['ArrowRight']
				}
			}
		}
	}
};

export type ShortcutKeybinds = {
	[C in ShortcutCategories]: {
		description: string;
		shortcuts: {
			action: string;
			keys: {
				[K in OperatingSystem | 'all']?: string[];
			};
			icons: {
				[K in OperatingSystem | 'all']?: string[];
			};
		}[];
	};
};

//data being re-arranged for keybindings page
export const keybindingsData = () => {
	let shortcuts = {} as ShortcutKeybinds;
	for (const category in ShortcutState['shortcuts']) {
		const shortcutCategory = ShortcutState['shortcuts'][category as ShortcutCategories] as ShortcutCategory;
		const categoryShortcuts: Array<Shortcut> = [];

		for (const shortcut in shortcutCategory) {
			if (shortcut === 'description') continue;
			const { keys, icons, action } = shortcutCategory[shortcut as ShortcutKeys] ?? {};
			if (keys && icons && action) {
				const categoryShortcut = {
					icons,
					action,
					keys
				};
				categoryShortcuts.push(categoryShortcut);
			}
			shortcuts = {
				...shortcuts,
				[category]: {
					description: shortcutCategory.description,
					shortcuts: categoryShortcuts
				}
			};
		}
	}
	return shortcuts;
};

export type ShortcutCategories = keyof typeof ShortcutState['shortcuts'];
type GetShortcutKeys<Category extends ShortcutCategories> =
keyof (typeof ShortcutState)['shortcuts'][Category];
//Not all shortcuts share the same keys (shortcuts) so this needs to be done like this
//A union type of all categories would return the 'description' only
type ShortcutKeys = Exclude<
	GetShortcutKeys<'Pages'> | GetShortcutKeys<'Dialogs'> | GetShortcutKeys<'Explorer'>,
	'description'
>;

const shortcutsStore = valtioPersist('sd-shortcuts', ShortcutState);

export function useShortcutsStore() {
	return useSnapshot(shortcutsStore);
}

export function getShortcutsStore() {
	return shortcutsStore;
}

export const useShortcut = (shortcut: ShortcutKeys, func: (e: KeyboardEvent) => void) => {
	const os = useOperatingSystem();
	const shortcutsStore = useShortcutsStore();
	const { visible } = useRoutingContext();

	const triggeredShortcut = () => {
		const shortcuts = {} as Record<ShortcutKeys, string[]>;
		for (const category in shortcutsStore['shortcuts']) {
			const shortcutCategory = shortcutsStore['shortcuts'][category as ShortcutCategories];
			for (const shortcut in shortcutCategory) {
				if (shortcut === 'description') continue;
				const keys = shortcutCategory[shortcut as ShortcutKeys]?.keys;
				shortcuts[shortcut as ShortcutKeys] = (keys?.[os] || keys?.all) as string[];
			}
		}
		return shortcuts[shortcut] as string[];
	};

	useKeys(triggeredShortcut(), (e) => {
		if (!visible) return;
		return func(e);
	});
};
