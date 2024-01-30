import { valtioPersist } from '@sd/client';
import { modifierSymbols } from '@sd/ui';
import { useMemo } from 'react';
import { useKeys } from 'rooks';
import { useSnapshot } from 'valtio';
import { useRoutingContext } from '~/RoutingContext';
import { OperatingSystem } from '~/util/Platform';

import { useOperatingSystem } from './useOperatingSystem';

export type Shortcut = {
	action: string;
	keys: Partial<Record<OperatingSystem | 'all', string[]>>;
	icons: Partial<Record<OperatingSystem | 'all', string[]>>;
};

export type ShortcutCategory = {
	description: string;
	shortcuts: Record<string, Shortcut>;
};

export const shortcutCategories = {
	General: {
		description: 'General usage shortcuts',
		shortcuts: {
			newTab: {
				action: 'Open new tab',
				keys: {
					macOS: ['Meta', 'KeyT'],
					all: ['Control', 'KeyT']
				},
				icons: {
					macOS: [modifierSymbols.Meta.macOS as string, 'T'],
					all: [modifierSymbols.Control.Other, 'T']
				},
			},
			closeTab: {
				action: 'Close current tab',
				keys: {
					macOS: ['Meta', 'KeyW'],
					all: ['Control', 'KeyW']
				},
				icons: {
					macOS: [modifierSymbols.Meta.macOS as string, 'W'],
					all: [modifierSymbols.Control.Other, 'W']
				},
				},
				nextTab: {
					action: 'Switch to next tab',
					keys: {
						macOS: ['Meta', 'Alt', 'ArrowRight'],
						all: ['Control', 'Alt', 'ArrowRight']
					},
					icons: {
						macOS: [modifierSymbols.Meta.macOS as string, modifierSymbols.Alt.macOS as string, 'ArrowRight'],
						all: [modifierSymbols.Control.Other, modifierSymbols.Alt.Windows as string, 'ArrowRight']
					},
				},
				previousTab: {
					action: 'Switch to previous tab',
					keys: {
						macOS: ['Meta', 'Alt', 'ArrowLeft'],
						all: ['Control', 'Alt', 'ArrowLeft']
					},
					icons: {
						macOS: [modifierSymbols.Meta.macOS as string, modifierSymbols.Alt.macOS as string, 'ArrowLeft'],
						all: [modifierSymbols.Control.Other, 'ArrowLeft']
					},
					}
				},
			},
	Dialogs: {
		description: 'To perform actions and operations',
		shortcuts: {
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
		}
	},
	Pages: {
		description: 'Different pages in the app',
		shortcuts: {
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
		}
	},
	Explorer: {
		description: 'To navigate and interact with the file system',
		shortcuts: {
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
} satisfies Record<string, ShortcutCategory>;

export type ShortcutName = {
	[K in keyof typeof shortcutCategories]: keyof (typeof shortcutCategories)[K]['shortcuts'];
}[keyof typeof shortcutCategories];

export const shortcutsStore = valtioPersist('sd-shortcuts', shortcutCategories);

export const useShortcut = (shortcut: ShortcutName, func: (e: KeyboardEvent) => void) => {
	const os = useOperatingSystem(true);
	const categories = useSnapshot(shortcutsStore);
	const { visible } = useRoutingContext();

	const keys = useMemo(() => {
		if (!visible) return [];

		const category = Object.values(categories).find((category) =>
		Object.prototype.hasOwnProperty.call(category.shortcuts, shortcut)) as ShortcutCategory | undefined;
		const categoryShortcut = category?.shortcuts[shortcut];

		return categoryShortcut?.keys[os] ?? categoryShortcut?.keys.all ?? [];
	}, [categories, os, shortcut, visible]);

	useKeys(keys, (e) => {
		if (!visible) return;
		return func(e);
	});
};
