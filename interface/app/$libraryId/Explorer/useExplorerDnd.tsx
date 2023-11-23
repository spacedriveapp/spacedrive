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

import { useExplorerContext } from './Context';
import { getExplorerStore } from './store';
import { explorerDroppableSchema } from './useExplorerDroppable';
import { useExplorerSearchParams } from './util';

const getPaths = (items: ExplorerItem[]) => {
	const paths = items
		.map((item) => {
			const filePath = getItemFilePath(item);
			if (filePath && 'path' in filePath) return filePath.path;
		})
		.filter((path): path is string => Boolean(path));

	return paths;
};

const getPathIds = (items: ExplorerItem[]) => {
	const ids = items
		.map((item) => getIndexedItemFilePath(item)?.id)
		.filter((id): id is number => Boolean(id));

	return ids;
};

export const useExplorerDnd = () => {
	const explorer = useExplorerContext();

	const [{ path }] = useExplorerSearchParams();

	const cutFiles = useLibraryMutation('files.cutFiles');
	const cutEphemeralFiles = useLibraryMutation('ephemeralFiles.cutFiles');

	useDndMonitor({
		onDragStart: () => {
			if (explorer.selectedItems.size === 0) return;
			getExplorerStore().drag = {
				type: 'dragging',
				items: [...explorer.selectedItems],
				sourceParentPath: path ?? '/',
				sourceLocationId:
					explorer.parent?.type === 'Location' ? explorer.parent.location.id : undefined
			};
		},
		onDragEnd: async ({ over }) => {
			const { drag } = getExplorerStore();
			getExplorerStore().drag = null;

			if (!over || !drag || drag.type === 'touched') return;

			const drop = explorerDroppableSchema.parse(over.data.current);

			switch (drop.type) {
				case 'location': {
					if (drop.data) {
						if (drag.sourceLocationId === undefined) {
							const path = drop.data.path + drop.path;
							if (path === drag.sourceParentPath) return;

							const paths = getPaths(drag.items);

							cutEphemeralFiles.mutate({
								sources: paths,
								target_dir: path
							});

							return;
						}

						const locationId = drop.data.id;
						const { path } = drop;

						if (locationId === drag.sourceLocationId && path === drag.sourceParentPath)
							return;

						cutFiles.mutate({
							source_location_id: drag.sourceLocationId,
							sources_file_path_ids: getPathIds(drag.items),
							target_location_id: locationId,
							target_location_relative_directory_path: path
						});

						return;
					}

					const { path } = drop;
					if (path === drag.sourceParentPath) return;

					const _paths = drag.items.map(async (item) => {
						const filePath = getItemFilePath(item);
						if (!filePath) return;

						return 'path' in filePath
							? filePath.path
							: await libraryClient.query(['files.getPath', filePath.id]);
					});

					const paths = (await Promise.all(_paths)).filter((path): path is string =>
						Boolean(path)
					);

					cutEphemeralFiles.mutate({
						sources: paths,
						target_dir: path
					});

					break;
				}

				case 'explorer-item': {
					switch (drop.data.type) {
						case 'Path': {
							const { item } = drop.data;

							if (drag.sourceLocationId === undefined) {
								const path = await libraryClient.query(['files.getPath', item.id]);
								if (!path) return;

								cutEphemeralFiles.mutate({
									sources: getPaths(drag.items),
									target_dir: path
								});

								return;
							}

							const path = item.materialized_path + item.name + '/';
							if (path === drag.sourceParentPath) return;

							cutFiles.mutate({
								source_location_id: drag.sourceLocationId,
								sources_file_path_ids: getPathIds(drag.items),
								target_location_id: item.location_id,
								target_location_relative_directory_path: path
							});

							break;
						}

						case 'Location':
						case 'NonIndexedPath': {
							const { path } = drop.data.item;
							if (path === drag.sourceParentPath) return;

							cutEphemeralFiles.mutate({
								sources: getPaths(drag.items),
								target_dir: path
							});
						}
					}
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
