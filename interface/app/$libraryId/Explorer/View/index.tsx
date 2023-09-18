import { Columns, GridFour, MonitorPlay, Rows, type Icon } from '@phosphor-icons/react';
import clsx from 'clsx';
import {
	isValidElement,
	memo,
	useCallback,
	useEffect,
	useRef,
	useState,
	type HTMLAttributes,
	type PropsWithChildren,
	type ReactNode
} from 'react';
import { createPortal } from 'react-dom';
import { createSearchParams, useNavigate } from 'react-router-dom';
import { useKeys } from 'rooks';
import {
	getItemObject,
	isPath,
	useLibraryContext,
	useLibraryMutation,
	type ExplorerItem,
	type FilePath,
	type Location,
	type NonIndexedPathItem,
	type Object
} from '@sd/client';
import { ContextMenu, dialogManager, ModifierKeys, toast } from '@sd/ui';
import { Loader } from '~/components';
import { useOperatingSystem } from '~/hooks';
import { isNonEmpty } from '~/util';
import { usePlatform } from '~/util/Platform';

import CreateDialog from '../../settings/library/tags/CreateDialog';
import { useExplorerContext } from '../Context';
import { QuickPreview } from '../QuickPreview';
import { useQuickPreviewContext } from '../QuickPreview/Context';
import { getQuickPreviewStore, useQuickPreviewStore } from '../QuickPreview/store';
import { getExplorerStore } from '../store';
import { uniqueId } from '../util';
import { useExplorerViewContext, ViewContext, type ExplorerViewContext } from '../ViewContext';
import GridView from './GridView';
import ListView from './ListView';
import MediaView from './MediaView';

interface ViewItemProps extends PropsWithChildren, HTMLAttributes<HTMLDivElement> {
	data: ExplorerItem;
}

export const ViewItem = ({ data, children, ...props }: ViewItemProps) => {
	const explorer = useExplorerContext();
	const explorerView = useExplorerViewContext();

	const navigate = useNavigate();
	const { library } = useLibraryContext();
	const { openFilePaths } = usePlatform();

	const updateAccessTime = useLibraryMutation('files.updateAccessTime');

	useKeys(['Meta', 'ArrowUp'], async (e) => {
		e.stopPropagation();
		await onDoubleClick();
	});

	const onDoubleClick = async () => {
		const selectedItems = [...explorer.selectedItems];

		if (!isNonEmpty(selectedItems)) return;

		let itemIndex = 0;
		const items = selectedItems.reduce(
			(items, item, i) => {
				const sameAsClicked = uniqueId(data) === uniqueId(item);

				if (sameAsClicked) itemIndex = i;

				switch (item.type) {
					case 'Location': {
						items.locations.splice(sameAsClicked ? 0 : -1, 0, item.item);
						break;
					}
					case 'NonIndexedPath': {
						items.non_indexed.splice(sameAsClicked ? 0 : -1, 0, item.item);
						break;
					}
					default: {
						for (const filePath of item.type === 'Path'
							? [item.item]
							: item.item.file_paths) {
							if (isPath(item) && item.item.is_dir) {
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

		if (items.paths.length > 0 && !explorerView.isRenaming) {
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
					toast.error({ title: 'Failed to open file', body: `Error: ${error}.` });
				}
			} else if (explorer.settingsStore.openOnDoubleClick === 'quickPreview') {
				if (data.type !== 'Location' && !(isPath(data) && data.item.is_dir)) {
					getQuickPreviewStore().itemIndex = itemIndex;
					getQuickPreviewStore().open = true;
					return;
				}
			}
		}

		if (items.dirs.length > 0) {
			const [item] = items.dirs;
			if (item) {
				navigate({
					pathname: `../location/${item.location_id}`,
					search: createSearchParams({
						path: `${item.materialized_path}${item.name}/`
					}).toString()
				});
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
			const [non_indexed] = items.non_indexed;
			if (non_indexed) {
				navigate({
					search: createSearchParams({ path: non_indexed.path }).toString()
				});
				return;
			}
		}
	};

	return (
		<ContextMenu.Root
			trigger={
				<div onDoubleClick={onDoubleClick} {...props}>
					{children}
				</div>
			}
			onOpenChange={explorerView.setIsContextMenuOpen}
			disabled={explorerView.contextMenu === undefined}
			asChild={false}
			onMouseDown={(e) => e.stopPropagation()}
		>
			{explorerView.contextMenu}
		</ContextMenu.Root>
	);
};

export interface ExplorerViewProps
	extends Omit<
		ExplorerViewContext,
		'selectable' | 'isRenaming' | 'setIsRenaming' | 'setIsContextMenuOpen' | 'ref'
	> {
	className?: string;
	style?: React.CSSProperties;
	emptyNotice?: JSX.Element;
}

export default memo(({ className, style, emptyNotice, ...contextProps }: ExplorerViewProps) => {
	const explorer = useExplorerContext();
	const quickPreviewStore = useQuickPreviewStore();

	const quickPreview = useQuickPreviewContext();

	const { layoutMode } = explorer.useSettingsSnapshot();

	const ref = useRef<HTMLDivElement>(null);

	const [isContextMenuOpen, setIsContextMenuOpen] = useState(false);
	const [isRenaming, setIsRenaming] = useState(false);
	const [showLoading, setShowLoading] = useState(false);

	useKeyDownHandlers({
		disabled: isRenaming || quickPreviewStore.open
	});

	useEffect(() => {
		if (explorer.isFetchingNextPage) {
			const timer = setTimeout(() => setShowLoading(true), 100);
			return () => clearTimeout(timer);
		} else setShowLoading(false);
	}, [explorer.isFetchingNextPage]);

	return (
		<>
			<div
				ref={ref}
				style={style}
				className={clsx('h-full w-full', className)}
				onMouseDown={(e) => {
					if (e.button === 2 || (e.button === 0 && e.shiftKey)) return;

					explorer.resetSelectedItems();
				}}
			>
				{explorer.items === null || (explorer.items && explorer.items.length > 0) ? (
					<ViewContext.Provider
						value={{
							...contextProps,
							selectable:
								explorer.selectable &&
								!isContextMenuOpen &&
								!isRenaming &&
								(!quickPreviewStore.open || explorer.selectedItems.size === 1),
							setIsContextMenuOpen,
							isRenaming,
							setIsRenaming,
							ref
						}}
					>
						{layoutMode === 'grid' && <GridView />}
						{layoutMode === 'list' && <ListView />}
						{layoutMode === 'media' && <MediaView />}
						{showLoading && (
							<Loader className="fixed bottom-10 left-0 w-[calc(100%+180px)]" />
						)}
					</ViewContext.Provider>
				) : (
					emptyNotice
				)}
			</div>

			{quickPreview.ref && createPortal(<QuickPreview />, quickPreview.ref)}
		</>
	);
});

export const EmptyNotice = (props: { icon?: Icon | ReactNode; message?: ReactNode }) => {
	const { layoutMode } = useExplorerContext().useSettingsSnapshot();

	const emptyNoticeIcon = (icon?: Icon) => {
		const Icon =
			icon ??
			{
				grid: GridFour,
				media: MonitorPlay,
				columns: Columns,
				list: Rows
			}[layoutMode];

		return <Icon size={100} opacity={0.3} />;
	};

	return (
		<div className="flex flex-col items-center justify-center h-full text-ink-faint">
			{props.icon
				? isValidElement(props.icon)
					? props.icon
					: emptyNoticeIcon(props.icon as Icon)
				: emptyNoticeIcon()}

			<p className="mt-5 text-sm font-medium">
				{props.message !== undefined ? props.message : 'This list is empty'}
			</p>
		</div>
	);
};

const useKeyDownHandlers = ({ disabled }: { disabled: boolean }) => {
	const explorer = useExplorerContext();

	const os = useOperatingSystem();
	const { library } = useLibraryContext();
	const { openFilePaths } = usePlatform();

	const handleNewTag = useCallback(
		async (event: KeyboardEvent) => {
			const objects: Object[] = [];

			for (const item of explorer.selectedItems) {
				const object = getItemObject(item);
				if (!object) return;
				objects.push(object);
			}

			if (
				!isNonEmpty(objects) ||
				event.key.toUpperCase() !== 'N' ||
				!event.getModifierState(os === 'macOS' ? ModifierKeys.Meta : ModifierKeys.Control)
			)
				return;

			dialogManager.create((dp) => <CreateDialog {...dp} objects={objects} />);
		},
		[os, explorer.selectedItems]
	);

	const handleOpenShortcut = useCallback(
		async (event: KeyboardEvent) => {
			if (
				event.key.toUpperCase() !== 'O' ||
				!event.getModifierState(
					os === 'macOS' ? ModifierKeys.Meta : ModifierKeys.Control
				) ||
				!openFilePaths
			)
				return;

			const paths: number[] = [];

			for (const item of explorer.selectedItems)
				for (const path of item.type === 'Path'
					? [item.item]
					: item.type === 'Object'
					? item.item.file_paths
					: [])
					paths.push(path.id);

			if (!isNonEmpty(paths)) return;

			try {
				await openFilePaths(library.uuid, paths);
			} catch (error) {
				toast.error({ title: 'Failed to open file', body: `Error: ${error}.` });
			}
		},
		[os, library.uuid, openFilePaths, explorer.selectedItems]
	);

	const handleExplorerShortcut = useCallback(
		(event: KeyboardEvent) => {
			if (
				event.key.toUpperCase() !== 'I' ||
				!event.getModifierState(os === 'macOS' ? ModifierKeys.Meta : ModifierKeys.Control)
			)
				return;

			getExplorerStore().showInspector = !getExplorerStore().showInspector;
		},
		[os]
	);

	useEffect(() => {
		const handlers = [handleNewTag, handleOpenShortcut, handleExplorerShortcut];
		const handler = (event: KeyboardEvent) => {
			if (event.repeat || disabled) return;
			for (const handler of handlers) handler(event);
		};
		document.body.addEventListener('keydown', handler);
		return () => document.body.removeEventListener('keydown', handler);
	}, [disabled, handleNewTag, handleOpenShortcut, handleExplorerShortcut]);
};
