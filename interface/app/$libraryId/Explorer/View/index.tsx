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
import { createSearchParams, useNavigate } from 'react-router-dom';
import { ExplorerItem, isPath, useLibraryContext, useLibraryMutation } from '@sd/client';
import { ContextMenu, ModifierKeys, dialogManager } from '@sd/ui';
import { showAlertDialog } from '~/components';
import {
	ExplorerLayoutMode,
	getExplorerStore,
	useExplorerConfigStore,
	useOperatingSystem
} from '~/hooks';
import { usePlatform } from '~/util/Platform';
import CreateDialog from '../../settings/library/tags/CreateDialog';
import {
	ExplorerViewContext,
	ExplorerViewSelection,
	ExplorerViewSelectionChange,
	ViewContext,
	useExplorerViewContext
} from '../ViewContext';
import { getExplorerItemData, getItemFilePath } from '../util';
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

	const { openFilePath } = usePlatform();
	const updateAccessTime = useLibraryMutation('files.updateAccessTime');
	const filePath = getItemFilePath(data);

	const explorerConfig = useExplorerConfigStore();

	const onDoubleClick = () => {
		if (isPath(data) && data.item.is_dir) {
			navigate({
				pathname: `/${library.uuid}/location/${getItemFilePath(data)?.location_id}`,
				search: createSearchParams({
					path: `${data.item.materialized_path}${data.item.name}/`
				}).toString()
			});
		} else if (
			openFilePath &&
			filePath &&
			explorerConfig.openOnDoubleClick &&
			!explorerView.isRenaming
		) {
			if (data.type === 'Path' && data.item.object_id) {
				updateAccessTime.mutate(data.item.object_id);
			}

			openFilePath(library.uuid, [filePath.id]);
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
			disabled={!explorerView.contextMenu}
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
	layout: ExplorerLayoutMode;
	className?: string;
	emptyNotice?: JSX.Element | { icon?: Icon | ReactNode; message?: ReactNode } | null;
}

export default memo(
	<T extends ExplorerViewSelection>({
		layout,
		className,
		emptyNotice,
		...contextProps
	}: ExplorerViewProps<T>) => {
		const os = useOperatingSystem();
		const { library } = useLibraryContext();
		const { openFilePath } = usePlatform();
		const [isContextMenuOpen, setIsContextMenuOpen] = useState(false);
		const [isRenaming, setIsRenaming] = useState(false);
		const selectedItem = useMemo(
			() =>
				contextProps.items?.find(
					(item) =>
						item.item.id ===
						(Array.isArray(contextProps.selected)
							? contextProps.selected[0]
							: contextProps.selected)
				),
			[contextProps.items, contextProps.selected]
		);
		const itemPath = selectedItem ? getItemFilePath(selectedItem) : null;

		const handleNewTag = useCallback(
			async (event: KeyboardEvent) => {
				if (
					itemPath == null ||
					event.key.toUpperCase() !== 'N' ||
					!event.getModifierState(
						os === 'macOS' ? ModifierKeys.Meta : ModifierKeys.Control
					)
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
					openFilePath == null ||
					event.key.toUpperCase() !== 'O' ||
					!event.getModifierState(
						os === 'macOS' ? ModifierKeys.Meta : ModifierKeys.Control
					)
				)
					return;

				try {
					await openFilePath(library.uuid, [itemPath.id]);
				} catch (error) {
					showAlertDialog({
						title: 'Error',
						value: `Couldn't open file, due to an error: ${error}`
					});
				}
			},
			[os, itemPath, library.uuid, openFilePath]
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
					!event.getModifierState(
						os === 'macOS' ? ModifierKeys.Meta : ModifierKeys.Control
					)
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

		const emptyNoticeIcon = (icon?: Icon) => {
			let Icon = icon;

			if (!Icon) {
				switch (layout) {
					case 'grid':
						Icon = GridFour;
						break;
					case 'media':
						Icon = MonitorPlay;
						break;
					case 'columns':
						Icon = Columns;
						break;
					case 'rows':
						Icon = Rows;
						break;
				}
			}

			return <Icon size={100} opacity={0.3} />;
		};

		return (
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
						{layout === 'grid' && <GridView />}
						{layout === 'rows' && <ListView />}
						{layout === 'media' && <MediaView />}
					</ViewContext.Provider>
				) : emptyNotice === null ? null : isValidElement(emptyNotice) ? (
					emptyNotice
				) : (
					<div className="flex h-full flex-col items-center justify-center text-ink-faint">
						{emptyNotice && 'icon' in emptyNotice
							? isValidElement(emptyNotice.icon)
								? emptyNotice.icon
								: emptyNoticeIcon(emptyNotice.icon as Icon)
							: emptyNoticeIcon()}

						<p className="mt-5 text-xs">
							{emptyNotice && 'message' in emptyNotice
								? emptyNotice.message
								: 'This list is empty'}
						</p>
					</div>
				)}
			</div>
		);
	}
) as <T extends ExplorerViewSelection>(props: ExplorerViewProps<T>) => JSX.Element;
