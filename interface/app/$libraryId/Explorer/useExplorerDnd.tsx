import { useDndMonitor } from '@dnd-kit/core';
import { useState } from 'react';
import {
	ExplorerItem,
	getIndexedItemFilePath,
	getItemFilePath,
	libraryClient,
	useLibraryMutation,
	useZodForm
} from '@sd/client';
import { Dialog, RadixCheckbox, useDialog, UseDialogProps } from '@sd/ui';
import { Icon } from '~/components';

import { useAssignItemsToTag } from '../settings/library/tags/CreateDialog';
import { useExplorerContext } from './Context';
import { getExplorerStore } from './store';
import { explorerDroppableSchema } from './useExplorerDroppable';
import { useExplorerSearchParams } from './util';

const getPaths = async (items: ExplorerItem[]) => {
	const paths = items.map(async (item) => {
		const filePath = getItemFilePath(item);
		if (!filePath) return;

		return 'path' in filePath
			? filePath.path
			: await libraryClient.query(['files.getPath', filePath.id]);
	});

	return (await Promise.all(paths)).filter((path): path is string => Boolean(path));
};

const getPathIds = (items: ExplorerItem[]) => {
	const ids = items
		.map((item) => getIndexedItemFilePath(item)?.id)
		.filter((id): id is number => Boolean(id));

	return ids;
};

const getObjectsPerLocation = (items: ExplorerItem[]) => {
	return items.reduce(
		(items, item) => {
			if (item.type !== 'Object') return items;

			const locationId = getIndexedItemFilePath(item)?.location_id;
			if (typeof locationId !== 'number') return items;

			return {
				...items,
				[locationId]: [...(items[locationId] ?? []), item]
			};
		},
		{} as Record<number, ExplorerItem[]>
	);
};

export const useExplorerDnd = () => {
	const explorer = useExplorerContext();

	const [{ path }] = useExplorerSearchParams();

	const cutFiles = useLibraryMutation('files.cutFiles');
	const cutEphemeralFiles = useLibraryMutation('ephemeralFiles.cutFiles');
	const assignItemsToTag = useAssignItemsToTag();

	useDndMonitor({
		onDragStart: () => {
			if (explorer.selectedItems.size === 0) return;
			getExplorerStore().drag = {
				type: 'dragging',
				items: [...explorer.selectedItems],
				sourcePath: path ?? '/',
				sourceLocationId:
					explorer.parent?.type === 'Location' ? explorer.parent.location.id : undefined,
				sourceTagId: explorer.parent?.type === 'Tag' ? explorer.parent.tag.id : undefined
			};
		},
		onDragEnd: async ({ over }) => {
			const { drag } = getExplorerStore();
			getExplorerStore().drag = null;

			if (!over || !drag || drag.type === 'touched') return;

			const drop = explorerDroppableSchema.parse(over.data.current);

			switch (drop.type) {
				case 'location': {
					// Drag from Ephemeral to Ephemeral
					if (!drop.data) {
						cutEphemeralFiles.mutate({
							sources: await getPaths(drag.items),
							target_dir: drop.path
						});

						return;
					}

					// Drag from Tag to Location
					if (drag.sourceTagId !== undefined) {
						const locationId = drop.data.id;

						const items = getObjectsPerLocation(drag.items);

						Object.entries(items).map(([sourceLocationId, items]) => {
							cutFiles.mutate({
								source_location_id: Number(sourceLocationId),
								sources_file_path_ids: getPathIds(items),
								target_location_id: locationId,
								target_location_relative_directory_path: drop.path
							});
						});

						return;
					}

					// Drag from Ephemeral to Location
					if (drag.sourceLocationId === undefined) {
						cutEphemeralFiles.mutate({
							sources: await getPaths(drag.items),
							target_dir: drop.data.path + drop.path
						});

						return;
					}

					// Drag between Locations
					cutFiles.mutate({
						source_location_id: drag.sourceLocationId,
						sources_file_path_ids: getPathIds(drag.items),
						target_location_id: drop.data.id,
						target_location_relative_directory_path: drop.path
					});

					break;
				}

				case 'explorer-item': {
					switch (drop.data.type) {
						case 'Path':
						case 'Object': {
							const { item } = drop.data;

							const filePath = 'file_paths' in item ? item.file_paths[0] : item;
							if (!filePath) return;

							if (drag.sourceTagId !== undefined) {
								const locationId = filePath.location_id;
								const path = filePath.materialized_path + filePath.name + '/';

								const items = getObjectsPerLocation(drag.items);

								Object.entries(items).map(([sourceLocationId, items]) => {
									cutFiles.mutate({
										source_location_id: Number(sourceLocationId),
										sources_file_path_ids: getPathIds(items),
										target_location_id: locationId,
										target_location_relative_directory_path: path
									});
								});

								return;
							}

							if (drag.sourceLocationId === undefined) {
								const path = await libraryClient.query([
									'files.getPath',
									filePath.id
								]);

								if (!path) return;

								cutEphemeralFiles.mutate({
									sources: await getPaths(drag.items),
									target_dir: path
								});

								return;
							}

							const locationId = filePath.location_id;
							const path = filePath.materialized_path + filePath.name + '/';

							if (drag.sourceLocationId === locationId && drag.sourcePath === path)
								return;

							cutFiles.mutate({
								source_location_id: drag.sourceLocationId,
								sources_file_path_ids: getPathIds(drag.items),
								target_location_id: locationId,
								target_location_relative_directory_path: path
							});

							break;
						}

						case 'Location':
						case 'NonIndexedPath': {
							cutEphemeralFiles.mutate({
								sources: await getPaths(drag.items),
								target_dir: drop.data.item.path
							});
						}
					}

					break;
				}

				case 'tag': {
					const items = drag.items.flatMap((item) => {
						if (item.type !== 'Object' && item.type !== 'Path') return [];
						return [item];
					});
					await assignItemsToTag(drop.data.id, items);
				}
			}
		},
		onDragCancel: () => (getExplorerStore().drag = null)
	});
};

interface DndNoticeProps extends UseDialogProps {
	count: number;
	path: string;
	onConfirm: (val: { dismissNotice: boolean }) => void;
}

const DndNotice = (props: DndNoticeProps) => {
	const form = useZodForm();
	const [dismissNotice, setDismissNotice] = useState(false);

	return (
		<Dialog
			form={form}
			onSubmit={form.handleSubmit(() => props.onConfirm({ dismissNotice: dismissNotice }))}
			dialog={useDialog(props)}
			title="Move Files"
			icon={<Icon name="MoveLocation" size={28} />}
			description={
				<span className="break-all">
					Are you sure you want to move {props.count} file{props.count > 1 ? 's' : ''} to{' '}
					{props.path}?
				</span>
			}
			ctaDanger
			ctaLabel="Continue"
			closeLabel="Cancel"
			buttonsSideContent={
				<RadixCheckbox
					label="Don't show again"
					name="ephemeral-alert-notice"
					checked={dismissNotice}
					onCheckedChange={(val) => typeof val === 'boolean' && setDismissNotice(val)}
				/>
			}
		/>
	);
};
