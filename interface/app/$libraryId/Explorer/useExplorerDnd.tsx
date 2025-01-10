import { useDndMonitor } from '@dnd-kit/core';
import {
	ExplorerItem,
	getIndexedItemFilePath,
	getItemFilePath,
	libraryClient,
	useLibraryMutation
} from '@sd/client';
import { isNonEmptyObject } from '~/util';

import { useAssignItemsToTag } from '../settings/library/tags/CreateDialog';
import { useExplorerContext } from './Context';
import { explorerStore } from './store';
import { explorerDroppableSchema } from './useExplorerDroppable';
import { getPathIdsPerLocation, useExplorerSearchParams } from './util';

export const getPaths = async (items: ExplorerItem[]) => {
	const paths = items.map(async (item) => {
		const filePath = getItemFilePath(item);
		if (!filePath) return;

		return 'path' in filePath
			? filePath.path
			: await libraryClient.query(['files.getPath', filePath.id]);
	});

	return (await Promise.all(paths)).filter((path): path is string => Boolean(path));
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
			explorerStore.drag = {
				type: 'dragging',
				items: [...explorer.selectedItems],
				sourcePath: path ?? '/',
				sourceLocationId:
					explorer.parent?.type === 'Location' ? explorer.parent.location.id : undefined,
				sourceTagId: explorer.parent?.type === 'Tag' ? explorer.parent.tag.id : undefined
			};
		},
		onDragEnd: async ({ over }) => {
			const drag = explorerStore.drag;
			explorerStore.drag = null;

			if (!over || !drag || drag.type === 'touched') return;

			const drop = explorerDroppableSchema.parse(over.data.current);

			switch (drop.type) {
				case 'location': {
					if (!drop.data) {
						cutEphemeralFiles.mutate({
							sources: await getPaths(drag.items),
							target_dir: drop.path
						});

						return;
					}

					const paths = getPathIdsPerLocation(drag.items);
					if (isNonEmptyObject(paths)) {
						const locationId = drop.data.id;

						Object.entries(paths).map(([sourceLocationId, paths]) => {
							cutFiles.mutate({
								source_location_id: Number(sourceLocationId),
								sources_file_path_ids: paths,
								target_location_id: locationId,
								target_location_relative_directory_path: drop.path
							});
						});

						return;
					}

					cutEphemeralFiles.mutate({
						sources: await getPaths(drag.items),
						target_dir: drop.data.path + drop.path
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

							const paths = getPathIdsPerLocation(drag.items);
							if (isNonEmptyObject(paths)) {
								const locationId = filePath.location_id;
								const path = filePath.materialized_path + filePath.name + '/';

								Object.entries(paths).map(([sourceLocationId, paths]) => {
									cutFiles.mutate({
										source_location_id: Number(sourceLocationId),
										sources_file_path_ids: paths,
										target_location_id: locationId,
										target_location_relative_directory_path: path
									});
								});

								return;
							}

							const path = await libraryClient.query(['files.getPath', filePath.id]);
							if (!path) return;

							cutEphemeralFiles.mutate({
								sources: await getPaths(drag.items),
								target_dir: path
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
		onDragCancel: () => (explorerStore.drag = null)
	});
};

// interface DndNoticeProps extends UseDialogProps {
// 	count: number;
// 	path: string;
// 	onConfirm: (val: { dismissNotice: boolean }) => void;
// }

// const DndNotice = (props: DndNoticeProps) => {
// 	const form = useZodForm();
// 	const [dismissNotice, setDismissNotice] = useState(false);

// 	const { t } = useLocale();

// 	return (
// 		<Dialog
// 			form={form}
// 			onSubmit={form.handleSubmit(() => props.onConfirm({ dismissNotice: dismissNotice }))}
// 			dialog={useDialog(props)}
// 			title={t('move_files')}
// 			icon={<Icon name="MoveLocation" size={28} />}
// 			description={
// 				<span className="break-all">
// 					Are you sure you want to move {props.count} file{props.count > 1 ? 's' : ''} to{' '}
// 					{props.path}?
// 				</span>
// 			}
// 			ctaDanger
// 			ctaLabel={t('continue')}
// 			closeLabel={t('cancel')}
// 			buttonsSideContent={
// 				<RadixCheckbox
// 					label={t('dont_show_again')}
// 					name="ephemeral-alert-notice"
// 					checked={dismissNotice}
// 					onCheckedChange={(val) => typeof val === 'boolean' && setDismissNotice(val)}
// 				/>
// 			}
// 		/>
// 	);
// };
