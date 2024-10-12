import type { ExplorerItem, FilePath, Location, NonIndexedPathItem } from '@sd/client';
import type { HTMLAttributes, PropsWithChildren } from 'react';

import { useCallback, useEffect } from 'react';
import {
	createSearchParams,
	useNavigate,
	useSearchParams as useRawSearchParams
} from 'react-router-dom';

import { isPath, SearchFilterArgs, useLibraryContext, useLibraryMutation } from '@sd/client';
import { ContextMenu, toast } from '@sd/ui';
import { useLocale } from '~/hooks';
import { isNonEmpty } from '~/util';
import { usePlatform } from '~/util/Platform';

import { useExplorerContext } from '../Context';
import { getQuickPreviewStore } from '../QuickPreview/store';
import { explorerStore } from '../store';
import { uniqueId } from '../util';
import { useExplorerViewContext } from './Context';

export const useViewItemDoubleClick = () => {
	const navigate = useNavigate();
	const explorer = useExplorerContext();
	const { library } = useLibraryContext();
	const { openFilePaths, openEphemeralFiles } = usePlatform();
	const [searchParams] = useRawSearchParams();

	const updateAccessTime = useLibraryMutation('files.updateAccessTime');

	const { t } = useLocale();

	const doubleClick = useCallback(
		async (item?: ExplorerItem) => {
			const selectedItems = [...explorer.selectedItems];

			if (!isNonEmpty(selectedItems)) return;

			let itemIndex = 0;
			const items = selectedItems.reduce(
				(items, selectedItem, i) => {
					const sameAsClicked = item && uniqueId(item) === uniqueId(selectedItem);

					if (sameAsClicked) itemIndex = i;

					switch (selectedItem.type) {
						case 'Location': {
							items.locations.splice(sameAsClicked ? 0 : -1, 0, selectedItem.item);
							break;
						}
						case 'NonIndexedPath': {
							items.non_indexed.splice(sameAsClicked ? 0 : -1, 0, selectedItem.item);
							break;
						}
						case 'SpacedropPeer':
						case 'Label':
							break;
						default: {
							const paths =
								selectedItem.type === 'Path'
									? [selectedItem.item]
									: selectedItem.item.file_paths;

							for (const filePath of paths) {
								if (filePath.is_dir) {
									items.dirs.splice(sameAsClicked ? 0 : -1, 0, filePath);
								} else {
									items.paths.splice(sameAsClicked ? 0 : -1, 0, filePath);
								}
							}
							break;
						}
					}

					return items;
				},
				{
					dirs: [],
					paths: [],
					locations: [],
					non_indexed: []
				} as {
					dirs: FilePath[];
					paths: FilePath[];
					locations: Location[];
					non_indexed: NonIndexedPathItem[];
				}
			);

			if (items.paths.length > 0) {
				if (explorer.settingsStore.openOnDoubleClick === 'openFile' && openFilePaths) {
					updateAccessTime
						.mutateAsync(items.paths.map(({ object_id }) => object_id!).filter(Boolean))
						.catch(console.error);

					try {
						await openFilePaths(
							library.uuid,
							items.paths.map(({ id }) => id)
						);
					} catch (error) {
						toast.error({
							title: t('failed_to_open_file_title'),
							body: t('error_message', { error })
						});
					}
				} else if (item && explorer.settingsStore.openOnDoubleClick === 'quickPreview') {
					if (item.type !== 'Location' && !(isPath(item) && item.item.is_dir)) {
						getQuickPreviewStore().itemIndex = itemIndex;
						getQuickPreviewStore().open = true;
						return;
					}
				}
			}

			if (items.dirs.length > 0) {
				const [item] = items.dirs;
				if (item) {
					if (item.location_id !== null) {
						const take = searchParams.get('take');
						const params = new URLSearchParams({
							path: `${item.materialized_path}${item.name}/`,
							...(take !== null && { take })
						});

						navigate(`/${library.uuid}/location/${item.location_id}?${params}`);
					}
					return;
				}
			}

			if (items.locations.length > 0) {
				const [location] = items.locations;
				if (location) {
					navigate({
						pathname: `../location/${location.id}`,
						search: createSearchParams({
							path: `/`
						}).toString()
					});
					return;
				}
			}

			if (items.non_indexed.length > 0) {
				if (items.non_indexed.length === 1) {
					const [non_indexed] = items.non_indexed;
					if (non_indexed && non_indexed.is_dir) {
						navigate({
							search: createSearchParams({ path: non_indexed.path }).toString()
						});
						return;
					}
				}

				if (explorer.settingsStore.openOnDoubleClick === 'openFile' && openEphemeralFiles) {
					try {
						await openEphemeralFiles(items.non_indexed.map(({ path }) => path));
					} catch (error) {
						toast.error({
							title: t('failed_to_open_file_title'),
							body: t('error_message', { error })
						});
					}
				} else if (item && explorer.settingsStore.openOnDoubleClick === 'quickPreview') {
					if (item.type !== 'Location' && !(isPath(item) && item.item.is_dir)) {
						getQuickPreviewStore().itemIndex = itemIndex;
						getQuickPreviewStore().open = true;
						return;
					}
				}
			}

			if (!item) return;

			if (item.type === 'Label') {
				navigate({
					pathname: '../search',
					search: createSearchParams({
						filters: JSON.stringify([
							{ object: { labels: { in: [item.item.id] } } }
						] as Array<SearchFilterArgs>)
					}).toString()
				});
				return;
			}
		},
		[
			explorer.selectedItems,
			explorer.settingsStore.openOnDoubleClick,
			openFilePaths,
			updateAccessTime,
			library.uuid,
			t,
			searchParams,
			navigate,
			openEphemeralFiles
		]
	);

	return { doubleClick };
};

interface ViewItemProps extends PropsWithChildren, HTMLAttributes<HTMLDivElement> {
	data: ExplorerItem;
}

export const ViewItem = ({ data, children, ...props }: ViewItemProps) => {
	const explorerView = useExplorerViewContext();

	const { doubleClick } = useViewItemDoubleClick();

	useEffect(() => {
		const handleContextMenu = (e: MouseEvent) => {
			e.preventDefault();
		};

		document.addEventListener('contextmenu', handleContextMenu);
		return () => {
			document.removeEventListener('contextmenu', handleContextMenu);
		};
	}, []);

	return (
		<ContextMenu.Root
			trigger={
				<div {...props} onDoubleClick={() => doubleClick(data)}>
					{children}
				</div>
			}
			onOpenChange={open => (explorerStore.isContextMenuOpen = open)}
			disabled={explorerView.contextMenu === undefined}
			onMouseDown={e => e.stopPropagation()}
		>
			{explorerView.contextMenu}
		</ContextMenu.Root>
	);
};
