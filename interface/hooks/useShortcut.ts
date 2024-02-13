import { useMemo } from 'react';
import { useKeys } from 'rooks';
import { useSnapshot } from 'valtio';
import { valtioPersist } from '@sd/client';
import { modifierSymbols } from '@sd/ui';
import i18n from '~/app/I18n';
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
	[i18n.t('general')]: {
		description: i18n.t('general_shortcut_description'),
		shortcuts: {
			newTab: {
				action: i18n.t('open_new_tab'),
				keys: {
					macOS: ['Meta', 'KeyT'],
					all: ['Control', 'KeyT']
				},
				icons: {
					macOS: [modifierSymbols.Meta.macOS as string, 'T'],
					all: [modifierSymbols.Control.Other, 'T']
				}
			},
			closeTab: {
				action: i18n.t('close_current_tab'),
				keys: {
					macOS: ['Meta', 'KeyW'],
					all: ['Control', 'KeyW']
				},
				icons: {
					macOS: [modifierSymbols.Meta.macOS as string, 'W'],
					all: [modifierSymbols.Control.Other, 'W']
				}
			},
			nextTab: {
				action: i18n.t('switch_to_next_tab'),
				keys: {
					macOS: ['Meta', 'Alt', 'ArrowRight'],
					all: ['Control', 'Alt', 'ArrowRight']
				},
				icons: {
					macOS: [
						modifierSymbols.Meta.macOS as string,
						modifierSymbols.Alt.macOS as string,
						'ArrowRight'
					],
					all: [modifierSymbols.Control.Other, modifierSymbols.Alt.Other, 'ArrowRight']
				}
			},
			previousTab: {
				action: i18n.t('switch_to_previous_tab'),
				keys: {
					macOS: ['Meta', 'Alt', 'ArrowLeft'],
					all: ['Control', 'Alt', 'ArrowLeft']
				},
				icons: {
					macOS: [
						modifierSymbols.Meta.macOS as string,
						modifierSymbols.Alt.macOS as string,
						'ArrowLeft'
					],
					all: [modifierSymbols.Control.Other, modifierSymbols.Alt.Other, 'ArrowLeft']
				}
			}
		}
	},
	[i18n.t('dialog')]: {
		description: i18n.t('dialog_shortcut_description'),
		shortcuts: {
			toggleJobManager: {
				action: i18n.t('toggle_job_manager'),
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
	[i18n.t('page')]: {
		description: i18n.t('page_shortcut_description'),
		shortcuts: {
			navBackwardHistory: {
				action: i18n.t('navigate_backwards'),
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
				action: i18n.t('navigate_forwards'),
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
				action: i18n.t('navigate_to_settings_page'),
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
	[i18n.t('explorer')]: {
		description: i18n.t('explorer_shortcut_description'),
		shortcuts: {
			gridView: {
				action: i18n.t('switch_to_grid_view'),
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
				action: i18n.t('switch_to_list_view'),
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
				action: i18n.t('switch_to_media_view'),
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
				action: i18n.t('toggle_hidden_files'),
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
				action: i18n.t('toggle_path_bar'),
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
				action: i18n.t('toggle_image_slider_within_quick_preview'),
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
				action: i18n.t('toggle_inspector'),
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
				action: i18n.t('toggle_quick_preview'),
				keys: {
					all: [' ']
				},
				icons: {
					all: [' ']
				}
			},
			toggleMetaData: {
				action: i18n.t('toggle_metadata'),
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
				action: i18n.t('move_back_within_quick_preview'),
				keys: {
					all: ['ArrowLeft']
				},
				icons: {
					all: ['ArrowLeft']
				}
			},
			quickPreviewMoveForward: {
				action: i18n.t('move_forward_within_quick_preview'),
				keys: {
					all: ['ArrowRight']
				},
				icons: {
					all: ['ArrowRight']
				}
			},
			revealNative: {
				action: i18n.t('reveal_in_native_file_manager'),
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
				action: i18n.t('rename_object'),
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
				action: i18n.t('rescan_location'),
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
				action: i18n.t('cut_object'),
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
				action: i18n.t('copy_object'),
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
				action: i18n.t('paste_object'),
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
				action: i18n.t('duplicate_object'),
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
				action: i18n.t('open_object'),
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
				action: i18n.t('open_object_from_quick_preview_in_native_file_manager'),
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
				action: i18n.t('delete_object'),
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
				action: i18n.t('cancel_selection'),
				keys: {
					all: ['Escape']
				},
				icons: {
					all: ['Escape']
				}
			},
			explorerDown: {
				action: i18n.t('navigate_files_downwards'),
				keys: {
					all: ['ArrowDown']
				},
				icons: {
					all: ['ArrowDown']
				}
			},
			explorerUp: {
				action: i18n.t('navigate_files_upwards'),
				keys: {
					all: ['ArrowUp']
				},
				icons: {
					all: ['ArrowUp']
				}
			},
			explorerLeft: {
				action: i18n.t('navigate_files_leftwards'),
				keys: {
					all: ['ArrowLeft']
				},
				icons: {
					all: ['ArrowLeft']
				}
			},
			explorerRight: {
				action: i18n.t('navigate_files_rightwards'),
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
			Object.prototype.hasOwnProperty.call(category.shortcuts, shortcut)
		) as ShortcutCategory | undefined;
		const categoryShortcut = category?.shortcuts[shortcut];

		return categoryShortcut?.keys[os] ?? categoryShortcut?.keys.all ?? [];
	}, [categories, os, shortcut, visible]);

	useKeys(keys, (e) => {
		if (!visible) return;
		e.preventDefault();
		return func(e);
	});
};
