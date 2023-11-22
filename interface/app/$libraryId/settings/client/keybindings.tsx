import {
	createColumnHelper,
	flexRender,
	getCoreRowModel,
	useReactTable
} from '@tanstack/react-table';
import clsx from 'clsx';
import { useState } from 'react';
import { Divider, ModifierKeys, modifierSymbols, Switch } from '@sd/ui';
import { useOperatingSystem } from '~/hooks';
import { keybindForOs } from '~/util/keybinds';
import { OperatingSystem } from '~/util/Platform';

import { Heading } from '../Layout';
import Setting from '../Setting';

type Shortcut = {
	action: string; //the name of the action the shortcut is performing
	description?: string; //what does this shortcut do?
	keys: {
		//the operating system the shortcut is for
		[K in OperatingSystem | 'all']?: {
			value: string | undefined | ModifierKeys | (string | undefined | ModifierKeys)[]; //if the shortcut is a single key, use a string, if it's a combination of keys, make it an array
			split?: boolean; //if the 'length' of the shortcut is >= 2, should it be split into 2 keys?
		};
	};
};

const shortcutCategories: Record<string, Shortcut[]> = {
	Pages: [
		{
			description: 'Different pages in the app',
			action: 'Navigate to Settings page',
			keys: {
				macOS: {
					value: [modifierSymbols.Shift.macOS, modifierSymbols.Meta.macOS, 'T']
				},
				all: {
					value: ['Shift', modifierSymbols.Control.Other, 'T']
				}
			}
		},
		{
			action: 'Navigate to Overview page',
			keys: {
				macOS: {
					value: [modifierSymbols.Shift.macOS, modifierSymbols.Meta.macOS, 'O']
				},
				all: {
					value: ['Shift', modifierSymbols.Control.Other, 'O']
				}
			}
		}
	],
	General: [
		{
			description: 'to perform general actions',
			action: 'Create new tab',
			keys: {
				macOS: {
					value: [modifierSymbols.Meta.macOS, 'T']
				},
				all: {
					value: [modifierSymbols.Control.Other, 'T']
				}
			}
		}
	],
	Dialogs: [
		{
			description: 'To perform actions and operations',
			action: 'Toggle Job Manager',
			keys: {
				macOS: {
					value: [modifierSymbols.Meta.macOS, 'J']
				},
				all: {
					value: [modifierSymbols.Control.Other, 'J']
				}
			}
		}
	],
	Explorer: [
		{
			action: 'Switch to grid view',
			keys: {
				macOS: {
					value: [modifierSymbols.Meta.macOS, '1']
				},
				all: {
					value: [modifierSymbols.Control.Other, '1']
				}
			}
		},
		{
			action: 'Switch to list view',
			keys: {
				macOS: {
					value: [modifierSymbols.Meta.macOS, '2']
				},
				all: {
					value: [modifierSymbols.Control.Other, '2']
				}
			}
		},
		{
			action: 'Switch to media view',
			keys: {
				macOS: {
					value: [modifierSymbols.Meta.macOS, '3']
				},
				all: {
					value: [modifierSymbols.Control.Other, '3']
				}
			}
		},
		{
			description: 'Where you explore your folders and files',
			action: 'Navigate explorer items',
			keys: {
				all: {
					value: ['ArrowUp', 'ArrowDown', 'ArrowRight', 'ArrowLeft']
				}
			}
		},
		{
			action: 'Navigate forward in folder history',
			keys: {
				macOS: {
					value: [modifierSymbols.Meta.macOS, ']']
				},
				all: {
					value: [modifierSymbols.Control.Other, ']']
				}
			}
		},
		{
			action: 'Navigate backward in folder history',
			keys: {
				macOS: {
					value: [modifierSymbols.Meta.macOS, '[']
				},
				all: {
					value: [modifierSymbols.Control.Other, '[']
				}
			}
		},
		{
			action: 'Delete selected item(s)',
			keys: {
				macOS: {
					value: [modifierSymbols.Meta.macOS, 'Backspace']
				},
				windows: {
					value: 'Del'
				}
			}
		},
		{
			action: 'Open selected item',
			keys: {
				macOS: {
					value: [modifierSymbols.Meta.macOS, 'O']
				},
				all: {
					value: [modifierSymbols.Control.Other, 'O']
				}
			}
		},
		{
			action: 'Show inspector',
			keys: {
				macOS: {
					value: [modifierSymbols.Meta.macOS, 'i']
				},
				all: {
					value: [modifierSymbols.Control.Other, 'i']
				}
			}
		},
		{
			action: 'Show path bar',
			keys: {
				macOS: {
					value: [modifierSymbols.Alt.macOS, modifierSymbols.Meta.macOS, 'p']
				},
				all: {
					value: [modifierSymbols.Alt.Other, modifierSymbols.Control.Other, 'p']
				}
			}
		},
		{
			action: 'Show image slider',
			keys: {
				macOS: {
					value: [modifierSymbols.Alt.macOS, modifierSymbols.Meta.macOS, 'm']
				},
				all: {
					value: [modifierSymbols.Alt.Other, modifierSymbols.Control.Other, 'm']
				}
			}
		},
		{
			action: 'Show hidden files',
			keys: {
				macOS: {
					value: [modifierSymbols.Meta.macOS, modifierSymbols.Shift.macOS, '.']
				},
				all: {
					value: [modifierSymbols.Control.Other, 'h']
				}
			}
		},
		{
			action: 'Copy selected item(s)',
			keys: {
				macOS: {
					value: [modifierSymbols.Meta.macOS, 'C']
				},
				all: {
					value: [modifierSymbols.Control.Other, 'C']
				}
			}
		},
		{
			action: 'Cut selected item(s)',
			keys: {
				macOS: {
					value: [modifierSymbols.Meta.macOS, 'X']
				},
				all: {
					value: [modifierSymbols.Control.Other, 'X']
				}
			}
		},
		{
			action: 'Paste selected item(s)',
			keys: {
				macOS: {
					value: [modifierSymbols.Meta.macOS, 'V']
				},
				all: {
					value: [modifierSymbols.Control.Other, 'V']
				}
			}
		},
		{
			action: 'Duplicate selected item(s)',
			keys: {
				macOS: {
					value: [modifierSymbols.Meta.macOS, 'D']
				},
				all: {
					value: [modifierSymbols.Control.Other, 'D']
				}
			}
		},
		{
			action: 'Reveal in Explorer/Finder',
			keys: {
				macOS: {
					value: [modifierSymbols.Meta.macOS, 'Y']
				},
				all: {
					value: [modifierSymbols.Control.Other, 'Y']
				}
			}
		},
		{
			action: 'Rescan',
			keys: {
				macOS: {
					value: [modifierSymbols.Meta.macOS, 'R']
				},
				all: {
					value: [modifierSymbols.Control.Other, 'R']
				}
			}
		},
		{
			action: 'Rename file or folder',
			keys: {
				windows: {
					value: 'F2',
					split: false
				},
				all: {
					value: 'Enter'
				}
			}
		},
		{
			action: 'Select first item in explorer',
			keys: {
				all: {
					value: 'ArrowDown'
				}
			}
		},
		{
			action: 'Open Quick Preview on selected item',
			keys: {
				all: {
					value: ' '
				}
			}
		}
	]
};

export const Component = () => {
	const [syncWithLibrary, setSyncWithLibrary] = useState(true);
	return (
		<>
			<Heading title="Keybinds" description="View and manage client keybinds" />{' '}
			<Setting
				mini
				title="Sync with Library"
				description="If enabled, your keybinds will be synced with library, otherwise they will apply only to this client."
			>
				<Switch
					checked={syncWithLibrary}
					onCheckedChange={setSyncWithLibrary}
					className="m-2 ml-4"
				/>
			</Setting>
			<Divider />
			{Object.entries(shortcutCategories).map(([category, shortcuts]) => {
				return (
					<div key={category} className="mb-4 space-y-0.5">
						<h1 className="inline-block text-lg font-bold text-ink">{category}</h1>
						<div className="pb-3">
							{shortcutCategories[category]?.map((category, i) => {
								return (
									<p className="text-sm text-ink-faint" key={i.toString()}>
										{category.description}
									</p>
								);
							})}
						</div>
						<KeybindTable data={shortcuts} />
					</div>
				);
			})}
		</>
	);
};

function KeybindTable({ data }: { data: Shortcut[] }) {
	const os = useOperatingSystem();
	const table = useReactTable({
		data,
		columns: createKeybindColumns(os),
		getCoreRowModel: getCoreRowModel()
	});

	return (
		<table className="w-full">
			<thead className="border-b border-b-app-line/30">
				{table.getHeaderGroups().map((headerGroup) => (
					<tr className="text-left" key={headerGroup.id}>
						{headerGroup.headers.map((header) => (
							<th className="pb-3 text-sm font-medium text-ink-dull" key={header.id}>
								{flexRender(header.column.columnDef.header, header.getContext())}
							</th>
						))}
					</tr>
				))}
			</thead>
			<tbody className="divide-y divide-app-line/30">
				{table.getRowModel().rows.map((row) => {
					return (
						<tr key={row.id}>
							{row.getVisibleCells().map((cell) => {
								return (
									<td
										className={clsx(
											'py-3 hover:brightness-125',
											cell.id.includes('key') ? 'w-fit text-right' : 'w-fit'
										)}
										key={cell.id}
									>
										{flexRender(cell.column.columnDef.cell, cell.getContext())}
									</td>
								);
							})}
						</tr>
					);
				})}
			</tbody>
		</table>
	);
}

function createKeybindColumns(os: OperatingSystem) {
	const keybind = keybindForOs(os);
	const columnHelper = createColumnHelper<Shortcut>();
	const columns = [
		columnHelper.accessor('action', {
			header: 'Description',
			cell: (info) => <p className="w-full text-sm text-ink-faint">{info.getValue()}</p>
		}),
		columnHelper.accessor('keys', {
			header: () => <p className="text-right">Key</p>,
			size: 200,
			cell: (info) => {
				const checkData = info.getValue()[os]?.value ?? info.getValue()['all']?.value;
				const data = Array.isArray(checkData) ? checkData : [checkData];
				const shortcuts = data.map((value) => {
					if (value) {
						const modifierKeyCheck = value in ModifierKeys ? [value] : [];
						return keybind(modifierKeyCheck as ModifierKeys[], [value]);
					}
				});
				return shortcuts.map((shortcut, idx) => {
					if (shortcut) {
						if (shortcut.length >= 2) {
							return (
								<div key={idx.toString()} className="inline-flex items-center">
									<kbd
										className="ml-2 rounded-lg border border-app-line bg-app-box px-2 py-1 text-[10.5px] tracking-widest shadow"
										key={idx.toString()}
									>
										{shortcut}
									</kbd>
								</div>
							);
						} else {
							return shortcut?.split(' ').map(([key], idx) => {
								return (
									<div key={idx.toString()} className="inline-flex items-center">
										<kbd
											className="ml-2 rounded-lg border border-app-line bg-app-box px-2 py-1 text-[10.5px] tracking-widest shadow"
											key={idx.toString()}
										>
											{key}
										</kbd>
									</div>
								);
							});
						}
					}
				});
			}
		})
	];
	return columns;
}
