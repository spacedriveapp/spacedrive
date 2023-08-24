import {
	createColumnHelper,
	flexRender,
	getCoreRowModel,
	useReactTable
} from '@tanstack/react-table';
import { useState } from 'react';
import { Divider, Switch } from '@sd/ui';
import { useOperatingSystem } from '~/hooks';
import { OperatingSystem } from '~/util/Platform';
import { Heading } from '../Layout';
import Setting from '../Setting';

type Shortcut = {
	action: string;
	key: {
		macOS: string[] | string[][];
		windows: string[] | string[][];
	};
};

// should this be an external JSON import?
const shortcutCategories: Record<string, Shortcut[]> = {
	Explorer: [
		{
			action: 'Navigate forward in folder history',
			key: {
				macOS: [
					['⌘', ']'],
					['⌘', '→']
				],
				windows: ['Alt', '→']
			}
		},
		{
			action: 'Navigate backward in folder history',
			key: {
				macOS: [
					['⌘', '['],
					['⌘', '←']
				],
				windows: ['Alt', '←']
			}
		},
		{
			action: 'Show enclosing folder',
			key: { macOS: ['⌘', '↑'], windows: ['Alt', '↑'] }
		},
		{
			action: 'Open selected item',
			key: {
				macOS: ['⌘', '↓'],
				windows: ['Alt', '↓']
			}
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
				<div key={category} className="mb-4 space-y-2">
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
								className={`py-2 ${
									cell.id.includes('key')
										? 'w-32 space-y-0.5 text-right'
										: 'w-full'
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
	function findPlatform(value: Shortcut['key'], os: OperatingSystem): string[][] {
		let keys;
		if (os === 'macOS') keys = value.macOS;
		else keys = value.windows;
		if (typeof keys[0] === 'string') return [keys as string[]];
		return keys as string[][];
	}

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
				const shortcuts = findPlatform(info.getValue(), os);
				return shortcuts.map((keys, i) => (
					<div key={i}>
						{keys.map((key, i) => (
							<>
								<kbd
									className="mb-2 rounded-lg border border-app-line bg-app-box px-2 py-1 text-sm shadow"
									key={key}
								>
									{key}
								</kbd>
								{i !== keys.length - 1 && (
									<span className="mx-1 font-thin text-ink-faint">+</span>
								)}
							</>
						))}
					</div>
				));
			}
		})
	];
	return columns;
}
