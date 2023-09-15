import {
	createColumnHelper,
	flexRender,
	getCoreRowModel,
	useReactTable
} from '@tanstack/react-table';
import { useState } from 'react';
import { Divider, ModifierKeys, Switch } from '@sd/ui';
import { useOperatingSystem } from '~/hooks';
import { keybindForOs } from '~/util/keybinds';
import { OperatingSystem } from '~/util/Platform';

import { Heading } from '../Layout';
import Setting from '../Setting';

type Shortcut = {
	action: string;
	key: [ModifierKeys[], string[]][];
};

const shortcutCategories: Record<string, Shortcut[]> = {
	Explorer: [
		{
			action: 'Navigate forward in folder history',
			key: [[[ModifierKeys.Control], ['ArrowRight']]]
		},
		{
			action: 'Navigate backward in folder history',
			key: [[[ModifierKeys.Control], ['ArrowLeft']]]
		},
		{
			action: 'Open Quick Preview on selected item',
			key: [[[], [' ']]]
		}
	]
};

export const Component = () => {
	const [syncWithLibrary, setSyncWithLibrary] = useState(true);

	return (
		<>
			{/* I don't care what you think the "right" way to write "keybinds" is, I simply refuse to refer to it as "keybindings" */}
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
			{Object.entries(shortcutCategories).map(([category, shortcuts]) => (
				<div key={category} className="mb-4 space-y-0.5">
					<h1 className="mb-3 inline-block text-lg font-bold text-ink">{category}</h1>
					<KeybindTable data={shortcuts} />
				</div>
			))}
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
			<thead>
				{table.getHeaderGroups().map((headerGroup) => (
					<tr className="text-left" key={headerGroup.id}>
						{headerGroup.headers.map((header) => (
							<th className="text-sm font-medium text-ink-dull" key={header.id}>
								{flexRender(header.column.columnDef.header, header.getContext())}
							</th>
						))}
					</tr>
				))}
			</thead>
			<tbody className="divide-y divide-app-line/60">
				{table.getRowModel().rows.map((row, i) => (
					<tr key={row.id}>
						{row.getVisibleCells().map((cell) => (
							<td
								className={`py-3 ${
									cell.id.includes('key') ? 'w-32 space-y-2 text-right' : 'w-full'
								}`}
								key={cell.id}
							>
								{flexRender(cell.column.columnDef.cell, cell.getContext())}
							</td>
						))}
					</tr>
				))}
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
				const shortcuts = info
					.getValue()
					.map(([modifiers, keys]) => keybind(modifiers, keys));
				return shortcuts.map((shortcut, i) => (
					<div key={i} className="inline-flex">
						{shortcut.split('').map((key, i) => {
							return (
								<>
									<kbd
										className="rounded-lg border border-app-line bg-app-box px-2 py-1 text-sm tracking-widest shadow"
										key={key}
									>
										{key}
									</kbd>
									{i !== shortcut.split('').length - 1 && (
										<span className="mx-1 font-thin text-ink-faint">+</span>
									)}
								</>
							);
						})}
					</div>
				));
			}
		})
	];
	return columns;
}
