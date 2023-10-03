import {
	createColumnHelper,
	flexRender,
	getCoreRowModel,
	useReactTable
} from '@tanstack/react-table';
import clsx from 'clsx';
import { useState } from 'react';
import { Divider, ModifierKeys, Switch } from '@sd/ui';
import { useOperatingSystem } from '~/hooks';
import { keybindForOs } from '~/util/keybinds';
import { OperatingSystem } from '~/util/Platform';

import { Heading } from '../Layout';
import Setting from '../Setting';

type Shortcut = {
	action: string;
	description?: string;
	key: {
		[K in OperatingSystem | 'all']?: {
			value: string | ModifierKeys | (string | ModifierKeys)[];
			split?: boolean;
		};
	};
};

const shortcutCategories: Record<string, Shortcut[]> = {
	Pages: [
		{
			description: 'Different pages in the app',
			action: 'Navigate to Settings page',
			key: {
				all: {
					value: ['G', 'S']
				}
			}
		}
	],
	Dialogs: [
		{
			description: 'To perform actions and operations',
			action: 'Toggle Job Manager',
			key: {
				all: {
					value: [ModifierKeys.Control, 'J']
				}
			}
		}
	],
	Explorer: [
		{
			description: 'Where you explore your folders and files',
			action: 'Navigate explorer items',
			key: {
				all: {
					value: ['ArrowUp', 'ArrowDown', 'ArrowRight', 'ArrowLeft']
				}
			}
		},
		{
			action: 'Navigate forward in folder history',
			key: {
				all: {
					value: [ModifierKeys.Control, 'ArrowRight']
				}
			}
		},
		{
			action: 'Navigate backward in folder history',
			key: {
				all: {
					value: [ModifierKeys.Control, 'ArrowLeft']
				}
			}
		},
		{
			action: 'Switch explorer layout',
			key: {
				all: {
					value: [ModifierKeys.Control, 'b']
				}
			}
		},
		{
			action: 'Open selected item',
			key: {
				all: {
					value: [ModifierKeys.Control, 'ArrowUp']
				}
			}
		},
		{
			action: 'Show inspector',
			key: {
				all: {
					value: [ModifierKeys.Control, 'i']
				}
			}
		},
		{
			action: 'Show path bar',
			key: {
				all: {
					value: [ModifierKeys.Control, 'p']
				}
			}
		},
		{
			action: 'Rename file or folder',
			key: {
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
			key: {
				all: {
					value: 'ArrowDown'
				}
			}
		},
		{
			action: 'Open Quick Preview on selected item',
			key: {
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
											cell.id.includes('key' || 'windowsKey')
												? 'w-fit text-right'
												: 'w-fit'
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
		columnHelper.accessor('key', {
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
						if (shortcut.length === 2) {
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
							return shortcut?.split(' ').map(([key]) => {
								const controlSymbolCheck =
									key === '⌘' ? (os === 'macOS' ? '⌘' : 'Ctrl') : key;
								return (
									<div key={idx.toString()} className="inline-flex items-center">
										<kbd
											className="ml-2 rounded-lg border border-app-line bg-app-box px-2 py-1 text-[10.5px] tracking-widest shadow"
											key={idx.toString()}
										>
											{controlSymbolCheck}
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
