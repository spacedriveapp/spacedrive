import { ColumnDef, flexRender, getCoreRowModel, useReactTable } from '@tanstack/react-table';
import clsx from 'clsx';
import { useMemo } from 'react';
import { keySymbols, modifierSymbols } from '@sd/ui';
import i18n from '~/app/I18n';
import { Shortcuts, shortcutsStore, useLocale, useOperatingSystem } from '~/hooks';

import { Heading } from '../Layout';

type ShortcutCategory = {
	name: string;
	description: string;
	shortcuts: { shortcut: Shortcuts; description: string }[];
};

export const Component = () => {
	const { t } = useLocale();

	// const [syncWithLibrary, setSyncWithLibrary] = useState(true);

	const categories = useMemo<ShortcutCategory[]>(
		() => [
			{
				name: t('general'),
				description: t('general_shortcut_description'),
				shortcuts: [
					{ shortcut: 'toggleCommandPalette', description: t('toggle_command_palette') },
					{ shortcut: 'closeCommandPalette', description: t('close_command_palette') },
					{ shortcut: 'newTab', description: t('open_new_tab') },
					{ shortcut: 'closeTab', description: t('close_current_tab') },
					{ shortcut: 'newTab', description: t('switch_to_next_tab') },
					{ shortcut: 'previousTab', description: t('switch_to_previous_tab') },
					{ shortcut: 'toggleSidebar', description: t('toggle_sidebar') },
					{ shortcut: 'duplicateTab', description: t('duplicate_current_tab') }
				]
			},
			{
				name: t('dialog'),
				description: t('dialog_shortcut_description'),
				shortcuts: [{ shortcut: 'toggleJobManager', description: t('toggle_job_manager') }]
			},
			{
				name: t('page'),
				description: t('page_shortcut_description'),
				shortcuts: [
					{ shortcut: 'navBackwardHistory', description: t('navigate_backwards') },
					{ shortcut: 'navForwardHistory', description: t('navigate_forwards') }
					// { shortcut: 'navToSettings', description: t('navigate_to_settings_page') }
				]
			},
			{
				name: t('explorer'),
				description: t('explorer_shortcut_description'),
				shortcuts: [
					{ shortcut: 'gridView', description: t('switch_to_grid_view') },
					{ shortcut: 'listView', description: t('switch_to_list_view') },
					{ shortcut: 'mediaView', description: t('switch_to_media_view') },
					{ shortcut: 'showHiddenFiles', description: t('toggle_hidden_files') },
					{ shortcut: 'showPathBar', description: t('toggle_path_bar') },
					{
						shortcut: 'showImageSlider',
						description: t('toggle_image_slider_within_quick_preview')
					},
					{ shortcut: 'showInspector', description: t('toggle_inspector') },
					{ shortcut: 'toggleQuickPreview', description: t('toggle_quick_preview') },
					{ shortcut: 'toggleMetaData', description: t('toggle_metadata') },
					{
						shortcut: 'quickPreviewMoveBack',
						description: t('move_back_within_quick_preview')
					},
					{
						shortcut: 'quickPreviewMoveForward',
						description: t('move_forward_within_quick_preview')
					},
					{
						shortcut: 'revealNative',
						description: t('reveal_in_native_file_manager')
					},
					{ shortcut: 'renameObject', description: t('rename_object') },
					{ shortcut: 'rescan', description: t('rescan_location') },
					{ shortcut: 'cutObject', description: t('cut_object') },
					{ shortcut: 'copyObject', description: t('copy_object') },
					{ shortcut: 'pasteObject', description: t('paste_object') },
					{ shortcut: 'duplicateObject', description: t('duplicate_object') },
					{ shortcut: 'openObject', description: t('open_object') },
					{
						shortcut: 'quickPreviewOpenNative',
						description: t('open_object_from_quick_preview_in_native_file_manager')
					},
					{ shortcut: 'delItem', description: t('delete_object') },
					{ shortcut: 'explorerEscape', description: t('cancel_selection') },
					{ shortcut: 'explorerDown', description: t('navigate_files_downwards') },
					{ shortcut: 'explorerUp', description: t('navigate_files_upwards') },
					{ shortcut: 'explorerLeft', description: t('navigate_files_leftwards') },
					{ shortcut: 'explorerRight', description: t('navigate_files_rightwards') }
				]
			}
		],
		[t]
	);

	return (
		<>
			<Heading title={t('keybinds')} description={t('keybinds_description')} />
			{/* <Setting
				mini
				title={t('sync_with_library')}
				description={t('sync_with_library_description')}
			>
				<Switch
					checked={syncWithLibrary}
					onCheckedChange={setSyncWithLibrary}
					className="m-2 ml-4"
				/>
			</Setting>
			<Divider /> */}

			{categories.map((category) => {
				return (
					<div key={category.name} className="mb-4 space-y-0.5">
						<h1 className="inline-block text-lg font-bold text-ink">{category.name}</h1>
						<div className="pb-3">
							<p className="text-sm text-ink-faint">{category.description}</p>
						</div>
						<KeybindTable data={category.shortcuts} />
					</div>
				);
			})}
		</>
	);
};

function KeybindTable({ data }: { data: ShortcutCategory['shortcuts'] }) {
	const os = useOperatingSystem(true);

	const columns = useMemo<ColumnDef<ShortcutCategory['shortcuts'][number]>[]>(
		() => [
			{
				accessorKey: 'description',
				header: i18n.t('description'),
				cell: (cell) => (
					<p className="w-full text-sm text-ink-faint">{`${cell.getValue()}`}</p>
				)
			},
			{
				accessorKey: 'shortcut',
				size: 200,
				header: () => <p className="text-right">{i18n.t('key')}</p>,
				cell: (cell) => {
					const shortcut = shortcutsStore[cell.row.original.shortcut];
					const keys = shortcut[os] ?? shortcut.all ?? [];

					// Modify OS as some symbol OSs are uppercase
					// TODO: Unify operating and symbol OS, besides 'Other'
					const symbolOS = os === 'windows' ? 'Windows' : os;

					const symbols = keys.map((key) => {
						if (key in modifierSymbols) {
							const symbol = modifierSymbols[key as keyof typeof modifierSymbols];
							return symbolOS in symbol
								? symbol[symbolOS as keyof typeof symbol]
								: symbol.Other;
						}

						if (key in keySymbols) {
							const symbol = keySymbols[key as keyof typeof keySymbols]!;
							return symbolOS in symbol
								? symbol[symbolOS as keyof typeof symbol]
								: symbol.Other;
						}

						// Check if shortcut key is prefixed with 'Key' (e.g.'KeyK')
						return key.startsWith('Key') ? key.split('Key')[1] : key;
					});

					return symbols.map((symbol) => (
						<div key={symbol} className="inline-flex items-center">
							<kbd className="ml-2 rounded-lg border border-app-line bg-app-box px-1.5 py-0.5 text-sm tracking-widest shadow">
								{symbol}
							</kbd>
						</div>
					));
				}
			}
		],
		[os]
	);

	const table = useReactTable({
		data,
		columns,
		getCoreRowModel: getCoreRowModel()
	});

	return (
		<table className="w-full">
			<thead className="border-b border-b-app-line/30">
				{table.getHeaderGroups().map((headerGroup) => (
					<tr key={headerGroup.id} className="text-left">
						{headerGroup.headers.map((header) => (
							<th key={header.id} className="pb-3 text-sm font-medium text-ink-dull">
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
										key={cell.id}
										className={clsx(
											'py-3 hover:brightness-125',
											cell.column.id === 'shortcut'
												? 'w-fit text-right'
												: 'w-fit'
										)}
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
