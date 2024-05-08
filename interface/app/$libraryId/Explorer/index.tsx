import { FolderNotchOpen } from '@phosphor-icons/react';
import { CSSProperties, type PropsWithChildren, type ReactNode } from 'react';
import {
	explorerLayout,
	useExplorerLayoutStore,
	useLibrarySubscription,
	useSelector
} from '@sd/client';
import { useShortcut } from '~/hooks';

import { useTopBarContext } from '../TopBar/Layout';
import { useExplorerContext } from './Context';
import ContextMenu from './ContextMenu';
import DismissibleNotice from './DismissibleNotice';
import { ExplorerPath, PATH_BAR_HEIGHT } from './ExplorerPath';
import { Inspector, INSPECTOR_WIDTH } from './Inspector';
import ExplorerContextMenu from './ParentContextMenu';
import { getQuickPreviewStore } from './QuickPreview/store';
import { explorerStore } from './store';
import { useKeyRevealFinder } from './useKeyRevealFinder';
import { ExplorerViewProps, View } from './View';
import { EmptyNotice } from './View/EmptyNotice';

import 'react-slidedown/lib/slidedown.css';

import clsx from 'clsx';

import { ExplorerTagBar } from './ExplorerTagBar';
import { useExplorerDnd } from './useExplorerDnd';

interface Props {
	emptyNotice?: ExplorerViewProps['emptyNotice'];
	contextMenu?: () => ReactNode;
}

/**
 * This component is used in a few routes and acts as the reference demonstration of how to combine
 * all the elements of the explorer except for the context, which must be used in the parent component.
 */
export default function Explorer(props: PropsWithChildren<Props>) {
	const explorer = useExplorerContext();
	const layoutStore = useExplorerLayoutStore();
	const [showInspector, showTagBar] = useSelector(explorerStore, (s) => [
		s.showInspector,
		s.tagAssignMode
	]);

	const showPathBar = explorer.showPathBar && layoutStore.showPathBar;

	// Can we put this somewhere else -_-
	useLibrarySubscription(['jobs.newThumbnail'], {
		onStarted: () => {
			console.log('Started RSPC subscription new thumbnail');
		},
		onError: (err) => {
			console.error('Error in RSPC subscription new thumbnail', err);
		},
		onData: (thumbKey) => {
			explorerStore.addNewThumbnail(thumbKey);
		}
	});

	useShortcut('showPathBar', (e) => {
		e.stopPropagation();
		explorerLayout.showPathBar = !layoutStore.showPathBar;
	});

	useShortcut('showInspector', (e) => {
		e.stopPropagation();
		if (getQuickPreviewStore().open) return;
		explorerStore.showInspector = !explorerStore.showInspector;
	});

	useShortcut('showHiddenFiles', (e) => {
		e.stopPropagation();
		explorer.settingsStore.showHiddenFiles = !explorer.settingsStore.showHiddenFiles;
	});

	useKeyRevealFinder();

	useExplorerDnd();

	const topBar = useTopBarContext();

	return (
		<>
			<ExplorerContextMenu>
				<div
					ref={explorer.scrollRef}
					className="custom-scroll explorer-scroll flex flex-1 flex-col overflow-x-hidden"
					style={
						{
							'--scrollbar-margin-top': `${topBar.topBarHeight}px`,
							'--scrollbar-margin-bottom': `${showPathBar ? PATH_BAR_HEIGHT : 0}px`,
							'paddingTop': topBar.topBarHeight,
							'paddingRight': showInspector ? INSPECTOR_WIDTH : 0
						} as CSSProperties
					}
				>
					{explorer.items && explorer.items.length > 0 && <DismissibleNotice />}

					<View
						contextMenu={props.contextMenu ? props.contextMenu() : <ContextMenu />}
						emptyNotice={
							props.emptyNotice ?? (
								<EmptyNotice
									icon={FolderNotchOpen}
									message="This folder is empty"
								/>
							)
						}
						listViewOptions={{ hideHeaderBorder: true }}
						scrollPadding={{
							top: topBar.topBarHeight,
							bottom: showPathBar ? PATH_BAR_HEIGHT : undefined
						}}
					/>
				</div>
			</ExplorerContextMenu>

			{/* TODO: wrap path bar and tag bar in nice wrapper, ideally animate tag bar in/out directly above path bar */}
			<div className="absolute inset-x-0 bottom-0 z-50 flex flex-col">
				{/* !!!! TODO: REMOVE BEFORE MERGE !!!! */}
				{/* !!!! TODO: REMOVE BEFORE MERGE !!!! */}
				{/* !!!! TODO: REMOVE BEFORE MERGE !!!! */}
				<button
					onClick={() => {
						explorerStore.tagAssignMode = !explorerStore.tagAssignMode;
					}}
				>
					DEBUG: Toggle tag assign mode
				</button>
				{/* !!!! TODO: REMOVE BEFORE MERGE !!!! */}
				{/* !!!! TODO: REMOVE BEFORE MERGE !!!! */}
				{/* !!!! TODO: REMOVE BEFORE MERGE !!!! */}
				{showTagBar && <ExplorerTagBar />}
				{showPathBar && <ExplorerPath />}
			</div>

			{showInspector && (
				<Inspector
					className={clsx(
						'no-scrollbar absolute right-1.5 top-0 pb-3 pl-3 pr-1.5',
						showPathBar && `b-[${PATH_BAR_HEIGHT}px]`
					)}
					style={{
						paddingTop: topBar.topBarHeight + 12
					}}
				/>
			)}
		</>
	);
}
