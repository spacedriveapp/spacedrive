import clsx from 'clsx';
import { Columns, GridFour, Icon, MonitorPlay, Rows } from 'phosphor-react';
import {
	HTMLAttributes,
	PropsWithChildren,
	ReactNode,
	isValidElement,
	memo,
	useCallback,
	useEffect,
	useMemo,
	useState
} from 'react';
import { createPortal } from 'react-dom';
import { createSearchParams, useNavigate } from 'react-router-dom';
import {
	ExplorerItem,
	getExplorerItemData,
	getItemFilePath,
	getItemLocation,
	isPath,
	useLibraryContext,
	useLibraryMutation
} from '@sd/client';
import { ContextMenu, ModifierKeys, dialogManager } from '@sd/ui';
import { showAlertDialog } from '~/components';
import { useOperatingSystem } from '~/hooks';
import { usePlatform } from '~/util/Platform';
import CreateDialog from '../../settings/library/tags/CreateDialog';
import { QuickPreview } from '../QuickPreview';
import { useQuickPreviewContext } from '../QuickPreview/Context';
import {
	ExplorerViewContext,
	ExplorerViewSelection,
	ExplorerViewSelectionChange,
	ViewContext,
	useExplorerViewContext
} from '../ViewContext';
import { useExplorerConfigStore } from '../config';
import { getExplorerStore, useExplorerStore } from '../store';
import GridView from './GridView';
import ListView from './ListView';
import MediaView from './MediaView';

interface ViewItemProps extends PropsWithChildren, HTMLAttributes<HTMLDivElement> {
	data: ExplorerItem;
}

export const ViewItem = ({ data, children, ...props }: ViewItemProps) => {
	const explorerView = useExplorerViewContext();
	const { library } = useLibraryContext();
	const navigate = useNavigate();

	const { openFilePaths } = usePlatform();
	const updateAccessTime = useLibraryMutation('files.updateAccessTime');
	const filePath = getItemFilePath(data);
	const location = getItemLocation(data);

	const explorerConfig = useExplorerConfigStore();

	const onDoubleClick = () => {
		if (location) {
			navigate({
				pathname: `/${library.uuid}/location/${location.id}`,
				search: createSearchParams({
					path: `/`
				}).toString()
			});
		} else if (isPath(data) && data.item.is_dir) {
			navigate({
				pathname: `/${library.uuid}/location/${getItemFilePath(data)?.location_id}`,
				search: createSearchParams({
					path: `${data.item.materialized_path}${data.item.name}/`
				}).toString()
			});
		} else if (
			openFilePaths &&
			filePath &&
			explorerConfig.openOnDoubleClick &&
			!explorerView.isRenaming
		) {
			if (data.type === 'Path' && data.item.object_id) {
				updateAccessTime.mutate(data.item.object_id);
			}

			openFilePaths(library.uuid, [filePath.id]);
		} else {
			const { kind } = getExplorerItemData(data);

			if (['Video', 'Image', 'Audio'].includes(kind)) {
				getExplorerStore().quickViewObject = data;
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
		>
			{explorerView.contextMenu}
		</ContextMenu.Root>
	);
};

export interface ExplorerViewProps<T extends ExplorerViewSelection = ExplorerViewSelection>
	extends Omit<
		ExplorerViewContext<T>,
		'multiSelect' | 'selectable' | 'isRenaming' | 'setIsRenaming' | 'setIsContextMenuOpen'
	> {
	className?: string;
	emptyNotice?: JSX.Element;
}

export default memo(
	<T extends ExplorerViewSelection>({
		className,
		emptyNotice,
		...contextProps
	}: ExplorerViewProps<T>) => {
		const { layoutMode } = useExplorerStore();

		const [isContextMenuOpen, setIsContextMenuOpen] = useState(false);
		const [isRenaming, setIsRenaming] = useState(false);

		useKeyDownHandlers({
			items: contextProps.items,
			selected: contextProps.selected,
			isRenaming
		});

		const quickPreviewCtx = useQuickPreviewContext();

		return (
			<>
				<div
					className={clsx('h-full w-full', className)}
					onMouseDown={() =>
						contextProps.onSelectedChange?.(
							(Array.isArray(contextProps.selected)
								? []
								: undefined) as ExplorerViewSelectionChange<T>
						)
					}
				>
					{contextProps.items === null ||
					(contextProps.items && contextProps.items.length > 0) ? (
						<ViewContext.Provider
							value={
								{
									...contextProps,
									multiSelect: Array.isArray(contextProps.selected),
									selectable: !isContextMenuOpen,
									setIsContextMenuOpen,
									isRenaming,
									setIsRenaming
								} as ExplorerViewContext
							}
						>
							{layoutMode === 'grid' && <GridView />}
							{layoutMode === 'rows' && <ListView />}
							{layoutMode === 'media' && <MediaView />}
						</ViewContext.Provider>
					) : (
						emptyNotice
					)}
				</div>

				{quickPreviewCtx.ref.current &&
					createPortal(<QuickPreview />, quickPreviewCtx.ref.current)}
			</>
		);
	}
) as <T extends ExplorerViewSelection>(props: ExplorerViewProps<T>) => JSX.Element;

export const EmptyNotice = ({
	icon,
	message
}: {
	icon?: Icon | ReactNode;
	message?: ReactNode;
}) => {
	const { layoutMode } = useExplorerStore();

	const emptyNoticeIcon = (icon?: Icon) => {
		const Icon =
			icon ??
			{
				grid: GridFour,
				media: MonitorPlay,
				columns: Columns,
				rows: Rows
			}[layoutMode];

		return <Icon size={100} opacity={0.3} />;
	};

	return (
		<div className="flex h-full flex-col items-center justify-center text-ink-faint">
			{icon
				? isValidElement(icon)
					? icon
					: emptyNoticeIcon(icon as Icon)
				: emptyNoticeIcon()}

			<p className="mt-5 text-sm font-medium">
				{message !== undefined ? message : 'This list is empty'}
			</p>
		</div>
	);
};

const useKeyDownHandlers = ({
	items,
	selected,
	isRenaming
}: Pick<ExplorerViewProps, 'items' | 'selected'> & { isRenaming: boolean }) => {
	const os = useOperatingSystem();
	const { library } = useLibraryContext();
	const { openFilePaths } = usePlatform();

	const selectedItem = useMemo(
		() =>
			items?.find(
				(item) => item.item.id === (Array.isArray(selected) ? selected[0] : selected)
			),
		[items, selected]
	);

	const itemPath = selectedItem ? getItemFilePath(selectedItem) : null;

	const handleNewTag = useCallback(
		async (event: KeyboardEvent) => {
			if (
				itemPath == null ||
				event.key.toUpperCase() !== 'N' ||
				!event.getModifierState(os === 'macOS' ? ModifierKeys.Meta : ModifierKeys.Control)
			)
				return;

			dialogManager.create((dp) => <CreateDialog {...dp} assignToObject={itemPath.id} />);
		},
		[os, itemPath]
	);

	const handleOpenShortcut = useCallback(
		async (event: KeyboardEvent) => {
			if (
				itemPath == null ||
				openFilePaths == null ||
				event.key.toUpperCase() !== 'O' ||
				!event.getModifierState(os === 'macOS' ? ModifierKeys.Meta : ModifierKeys.Control)
			)
				return;

			try {
				await openFilePaths(library.uuid, [itemPath.id]);
			} catch (error) {
				showAlertDialog({
					title: 'Error',
					value: `Couldn't open file, due to an error: ${error}`
				});
			}
		},
		[os, itemPath, library.uuid, openFilePaths]
	);

	const handleOpenQuickPreview = useCallback(
		async (event: KeyboardEvent) => {
			if (event.key !== ' ') return;
			if (!getExplorerStore().quickViewObject) {
				if (selectedItem) {
					getExplorerStore().quickViewObject = selectedItem;
				}
			} else {
				getExplorerStore().quickViewObject = null;
			}
		},
		[selectedItem]
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
