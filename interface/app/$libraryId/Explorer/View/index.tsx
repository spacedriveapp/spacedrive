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
	useRef,
	useState
} from 'react';
import { createPortal } from 'react-dom';
import { createSearchParams, useNavigate } from 'react-router-dom';
import {
	ExplorerItem,
	getExplorerItemData,
	getItemFilePath,
	getItemLocation,
	getItemObject,
	isPath,
	useLibraryContext,
	useLibraryMutation
} from '@sd/client';
import { ContextMenu, ModifierKeys, dialogManager } from '@sd/ui';
import { showAlertDialog } from '~/components';
import { useOperatingSystem } from '~/hooks';
import { usePlatform } from '~/util/Platform';
import CreateDialog from '../../settings/library/tags/CreateDialog';
import { useExplorerContext } from '../Context';
import { QuickPreview } from '../QuickPreview';
import { useQuickPreviewContext } from '../QuickPreview/Context';
import { ExplorerViewContext, ViewContext, useExplorerViewContext } from '../ViewContext';
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

	const { layoutMode } = useExplorerStore();

	const ref = useRef<HTMLDivElement>(null);

	const [isContextMenuOpen, setIsContextMenuOpen] = useState(false);
	const [isRenaming, setIsRenaming] = useState(false);

	useKeyDownHandlers({
		isRenaming
	});

	return (
		<>
			<div
				ref={ref}
				style={style}
				className={clsx('h-full w-full', className)}
				onMouseDown={(e) => {
					if (e.button === 2 || (e.button === 0 && e.shiftKey)) return;

					console.log('bruh');

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
});

export const EmptyNotice = (props: { icon?: Icon | ReactNode; message?: ReactNode }) => {
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
			{props.icon
				? isValidElement(props.icon)
					? props.icon
					: emptyNoticeIcon(props.icon as Icon)
				: emptyNoticeIcon()}

			<p className="mt-5 text-xs">
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

	/// TODO: This ain't right!
	const selectedItem = [...explorer.selectedItems][0];

	const itemPath = selectedItem ? getItemFilePath(selectedItem) : null;
	const object = selectedItem ? getItemObject(selectedItem) : null;

	const handleNewTag = useCallback(
		async (event: KeyboardEvent) => {
			if (
				object == null ||
				event.key.toUpperCase() !== 'N' ||
				!event.getModifierState(os === 'macOS' ? ModifierKeys.Meta : ModifierKeys.Control)
			)
				return;

			dialogManager.create((dp) => <CreateDialog {...dp} objects={[object]} />);
		},
		[os, object]
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
