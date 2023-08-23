import clsx from 'clsx';
import { Columns, GridFour, type Icon, MonitorPlay, Rows } from 'phosphor-react';
import {
	type HTMLAttributes,
	type PropsWithChildren,
	type ReactNode,
	isValidElement,
	memo,
	useCallback,
	useEffect,
	useRef,
	useState
} from 'react';
import { createPortal } from 'react-dom';
import { createSearchParams, useNavigate } from 'react-router-dom';
import {
	type ExplorerItem,
	type FilePath,
	type Location,
	type NonIndexedPathItem,
	type Object,
	getItemObject,
	isPath,
	useLibraryContext,
	useLibraryMutation
} from '@sd/client';
import { ContextMenu, ModifierKeys, dialogManager } from '@sd/ui';
import { showAlertDialog } from '~/components';
import { useOperatingSystem } from '~/hooks';
import { isNonEmpty } from '~/util';
import { usePlatform } from '~/util/Platform';
import CreateDialog from '../../settings/library/tags/CreateDialog';
import { useExplorerContext } from '../Context';
import { QuickPreview } from '../QuickPreview';
import { useQuickPreviewContext } from '../QuickPreview/Context';
import { type ExplorerViewContext, ViewContext, useExplorerViewContext } from '../ViewContext';
import { useExplorerConfigStore } from '../config';
import { getExplorerStore } from '../store';
import { uniqueId } from '../util';
import GridView from './GridView';
import ListView from './ListView';
import MediaView from './MediaView';

interface ViewItemProps extends PropsWithChildren, HTMLAttributes<HTMLDivElement> {
	data: ExplorerItem;
}

export const ViewItem = ({ data, children, ...props }: ViewItemProps) => {
	const explorer = useExplorerContext();
	const explorerView = useExplorerViewContext();

	const explorerConfig = useExplorerConfigStore();

	const navigate = useNavigate();
	const { library } = useLibraryContext();
	const { openFilePaths } = usePlatform();

	const updateAccessTime = useLibraryMutation('files.updateAccessTime');

	const onDoubleClick = async () => {
		const selectedItems = [...explorer.selectedItems].reduce(
			(items, item) => {
				const sameAsClicked = uniqueId(data) === uniqueId(item);

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

		if (selectedItems.paths.length > 0 && !explorerView.isRenaming) {
			if (explorerConfig.openOnDoubleClick && openFilePaths) {
				updateAccessTime
					.mutateAsync(
						selectedItems.paths.map(({ object_id }) => object_id!).filter(Boolean)
					)
					.catch(console.error);

				try {
					await openFilePaths(
						library.uuid,
						selectedItems.paths.map(({ id }) => id)
					);
				} catch (error) {
					showAlertDialog({
						title: 'Error',
						value: `Failed to open file, due to an error: ${error}`
					});
				}
			} else if (!explorerConfig.openOnDoubleClick) {
				if (data.type !== 'Location' && !(isPath(data) && data.item.is_dir)) {
					getExplorerStore().quickViewObject = data;
					return;
				}
			}
		}

		if (selectedItems.dirs.length > 0) {
			const [item] = selectedItems.dirs;
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

		if (selectedItems.locations.length > 0) {
			const [location] = selectedItems.locations;
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

		if (selectedItems.non_indexed.length > 0) {
			const [non_indexed] = selectedItems.non_indexed;
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

	const quickPreviewCtx = useQuickPreviewContext();

	const { layoutMode } = explorer.useSettingsSnapshot();

	const ref = useRef<HTMLDivElement>(null);

	const [isContextMenuOpen, setIsContextMenuOpen] = useState(false);
	const [isRenaming, setIsRenaming] = useState(false);

	useKeyDownHandlers({
		isRenaming
	});

	useEffect(() => {
		// using .next() is not great
		const explorerStore = getExplorerStore();
		const selectedItem = explorer.selectedItems.values().next().value as
			| ExplorerItem
			| undefined;
		if (explorerStore.quickViewObject != null && selectedItem) {
			explorerStore.quickViewObject = selectedItem;
		}
	}, [explorer.selectedItems]);

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
						value={
							{
								...contextProps,
								selectable:
									explorer.selectable && !isContextMenuOpen && !isRenaming,
								setIsContextMenuOpen,
								isRenaming,
								setIsRenaming,
								ref
							} as ExplorerViewContext
						}
					>
						{layoutMode === 'grid' && <GridView />}
						{layoutMode === 'list' && <ListView />}
						{layoutMode === 'media' && <MediaView />}
					</ViewContext.Provider>
				) : (
					emptyNotice
				)}
			</div>
			{quickPreviewCtx.ref && createPortal(<QuickPreview />, quickPreviewCtx.ref)}
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
		<div className="flex h-full flex-col items-center justify-center text-ink-faint">
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

const useKeyDownHandlers = ({ isRenaming }: { isRenaming: boolean }) => {
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
				event.code.toUpperCase() !== 'O' ||
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
				showAlertDialog({
					title: 'Error',
					value: `Couldn't open file, due to an error: ${error}`
				});
			}
		},
		[os, library.uuid, openFilePaths, explorer.selectedItems]
	);

	const handleOpenQuickPreview = useCallback(
		async (event: KeyboardEvent) => {
			if (event.key !== ' ') return;
			if (!getExplorerStore().quickViewObject) {
				// ENG-973 - Don't use Set -> Array -> First Item
				const items = [...explorer.selectedItems];
				if (!isNonEmpty(items)) return;

				getExplorerStore().quickViewObject = items[0];
			} else {
				getExplorerStore().quickViewObject = null;
			}
		},
		[explorer.selectedItems]
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
		const handlers = [
			handleNewTag,
			handleOpenShortcut,
			handleOpenQuickPreview,
			handleExplorerShortcut
		];
		const handler = (event: KeyboardEvent) => {
			if (isRenaming) return;
			for (const handler of handlers) handler(event);
		};
		document.body.addEventListener('keydown', handler);
		return () => document.body.removeEventListener('keydown', handler);
	}, [
		isRenaming,
		handleNewTag,
		handleOpenShortcut,
		handleOpenQuickPreview,
		handleExplorerShortcut
	]);
};
