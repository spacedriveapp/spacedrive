import {
	createColumnHelper,
	flexRender,
	getCoreRowModel,
	useReactTable
} from '@tanstack/react-table';
import clsx from 'clsx';
import { useState } from 'react';
import { Divider, ModifierKeys, Switch } from '@sd/ui';
import { keybindingsData, ShortcutCategories, ShortcutKeybinds, useOperatingSystem } from '~/hooks';
import { keybindForOs } from '~/util/keybinds';
import { OperatingSystem } from '~/util/Platform';

import { Heading } from '../Layout';
import Setting from '../Setting';

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
			{Object.entries(keybindingsData()).map(([category, info]) => {
				return (
					<div key={category} className="mb-4 space-y-0.5">
						<h1 className="inline-block text-lg font-bold text-ink">{category}</h1>
						<div className="pb-3">
							<p className="text-sm text-ink-faint">
								{keybindingsData()[category as ShortcutCategories]?.description}
							</p>
						</div>
						<KeybindTable data={info.shortcuts} />
					</div>
				);
			})}
		</>
	);
};

function KeybindTable({ data }: { data: ShortcutKeybinds[ShortcutCategories]['shortcuts'] }) {
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
											cell.id.includes('icon') ? 'w-fit text-right' : 'w-fit'
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
	const columnHelper = createColumnHelper<{
		action: string;
		icons: {
			[key in OperatingSystem | 'all']?: string | string[];
		};
	}>();
	const columns = [
		columnHelper.accessor('action', {
			header: 'Description',
			cell: (info) => <p className="w-full text-sm text-ink-faint">{info.getValue()}</p>
		}),
		columnHelper.accessor('icons', {
			header: () => <p className="text-right">Key</p>,
			size: 200,
			cell: (info) => {
				const checkData = info.getValue()[os] || info.getValue()['all'];
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
