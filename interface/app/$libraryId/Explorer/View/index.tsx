import clsx from 'clsx';
import { Columns, GridFour, Icon, MonitorPlay, Rows } from 'phosphor-react';
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
	type Object,
	getItemFilePath,
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

	function updateList<T = FilePath | Location>(list: T[], item: T, push: boolean) {
		return !push ? [item, ...list] : [...list, item];
	}

	const onDoubleClick = async () => {
		const selectedItems = [...explorer.selectedItems];

		if (!isNonEmpty(selectedItems)) return;

		const items = selectedItems.reduce(
			(items, item) => {
				const sameAsClicked = data.item.id === item.item.id;

				switch (item.type) {
					case 'Path':
					case 'Object': {
						const filePath = getItemFilePath(item);
						if (filePath) {
							if (isPath(item) && item.item.is_dir) {
								items.dirs = updateList(items.dirs, filePath, !sameAsClicked);
							} else items.paths = updateList(items.paths, filePath, !sameAsClicked);
						}
						break;
					}

					case 'Location': {
						items.locations = updateList(items.locations, item.item, !sameAsClicked);
					}
				}

				return items;
			},
			{
				paths: [],
				dirs: [],
				locations: []
			} as { paths: FilePath[]; dirs: FilePath[]; locations: Location[] }
		);

		if (items.paths.length > 0 && !explorerView.isRenaming) {
			if (explorerConfig.openOnDoubleClick && openFilePaths) {
				updateAccessTime
					.mutateAsync(items.paths.map(({ object_id }) => object_id!).filter(Boolean))
					.catch(console.error);

				try {
					await openFilePaths(
						library.uuid,
						items.paths.map(({ id }) => id)
					);
				} catch (error) {
					showAlertDialog({
						title: 'Error',
						value: `Failed to open file, due to an error: ${error}`
					});
				}
			} else if (!explorerConfig.openOnDoubleClick) {
				if (data.type !== 'Location' && !(isPath(data) && data.item.is_dir)) {
					getExplorerStore().showQuickView = true;
					return;
				}
			}
		}

		if (items.dirs.length > 0) {
			const item = items.dirs[0];
			if (!item) return;

			navigate({
				pathname: `../location/${item.location_id}`,
				search: createSearchParams({
					path: `${item.materialized_path}${item.name}/`
				}).toString()
			});
		} else if (items.locations.length > 0) {
			const location = items.locations[0];
			if (!location) return;

			navigate({
				pathname: `../location/${location.id}`,
				search: createSearchParams({
					path: `/`
				}).toString()
			});
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

	return (
		<ViewContext.Provider
			value={{
				...contextProps,
				selectable: explorer.selectable && !isContextMenuOpen && !isRenaming,
				setIsContextMenuOpen,
				isRenaming,
				setIsRenaming,
				ref
			}}
		>
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
					<>
						{layoutMode === 'grid' && <GridView />}
						{layoutMode === 'list' && <ListView />}
						{layoutMode === 'media' && <MediaView />}
					</>
				) : (
					emptyNotice
				)}
			</div>

			{quickPreviewCtx.ref && createPortal(<QuickPreview />, quickPreviewCtx.ref)}
		</ViewContext.Provider>
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

			for (const item of explorer.selectedItems) {
				const path = getItemFilePath(item);
				if (!path) return;
				paths.push(path.id);
			}

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

			event.preventDefault();

			if (!getExplorerStore().showQuickView) {
				if (explorer.selectedItems.size === 0) return;

				getExplorerStore().showQuickView = true;
			} else {
				getExplorerStore().showQuickView = false;
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
