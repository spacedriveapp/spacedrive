import { Columns, GridFour, MonitorPlay, Rows, type Icon } from '@phosphor-icons/react';
import clsx from 'clsx';
import {
	isValidElement,
	memo,
	useCallback,
	useEffect,
	useRef,
	useState,
	type ReactNode
} from 'react';
import { createPortal } from 'react-dom';
import { useKeys } from 'rooks';
import { getItemObject, useLibraryContext, type Object } from '@sd/client';
import { dialogManager, ModifierKeys, toast } from '@sd/ui';
import { Loader } from '~/components';
import { useKeyMatcher, useOperatingSystem } from '~/hooks';
import { isNonEmpty } from '~/util';
import { usePlatform } from '~/util/Platform';

import CreateDialog from '../../settings/library/tags/CreateDialog';
import { useExplorerContext } from '../Context';
import { QuickPreview } from '../QuickPreview';
import { useQuickPreviewContext } from '../QuickPreview/Context';
import { useQuickPreviewStore } from '../QuickPreview/store';
import { getExplorerStore } from '../store';
import { ViewContext, type ExplorerViewContext } from '../ViewContext';
import GridView from './GridView';
import ListView from './ListView';
import MediaView from './MediaView';
import { useExplorerViewPadding } from './util';
import { useViewItemDoubleClick } from './ViewItem';

export interface ExplorerViewPadding {
	x?: number;
	y?: number;
	top?: number;
	bottom?: number;
	left?: number;
	right?: number;
}

export interface ExplorerViewProps
	extends Omit<
		ExplorerViewContext,
		'selectable' | 'isRenaming' | 'setIsRenaming' | 'setIsContextMenuOpen' | 'ref' | 'padding'
	> {
	className?: string;
	style?: React.CSSProperties;
	emptyNotice?: JSX.Element;
	padding?: number | ExplorerViewPadding;
}

export default memo(
	({ className, style, emptyNotice, padding, ...contextProps }: ExplorerViewProps) => {
		const explorer = useExplorerContext();
		const quickPreview = useQuickPreviewContext();
		const quickPreviewStore = useQuickPreviewStore();

		const { doubleClick } = useViewItemDoubleClick();

		const { layoutMode } = explorer.useSettingsSnapshot();

		const metaCtrlKey = useKeyMatcher('Meta').key;

		const ref = useRef<HTMLDivElement>(null);

		const [isContextMenuOpen, setIsContextMenuOpen] = useState(false);
		const [isRenaming, setIsRenaming] = useState(false);
		const [showLoading, setShowLoading] = useState(false);

		const viewPadding = useExplorerViewPadding(padding);

		useKeyDownHandlers({
			disabled: isRenaming || quickPreviewStore.open
		});

		useEffect(() => {
			if (explorer.isFetchingNextPage) {
				const timer = setTimeout(() => setShowLoading(true), 100);
				return () => clearTimeout(timer);
			} else setShowLoading(false);
		}, [explorer.isFetchingNextPage]);

		useKeys([metaCtrlKey, 'ArrowUp'], (e) => {
			e.stopPropagation();
			doubleClick();
		});

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
								ref,
								padding: viewPadding
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
	}
);

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
