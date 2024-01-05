import { FolderNotchOpen } from '@phosphor-icons/react';
import { CSSProperties, type PropsWithChildren, type ReactNode } from 'react';
import {
	getExplorerLayoutStore,
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
import { explorerStore, getExplorerStore } from './store';
import { useKeyRevealFinder } from './useKeyRevealFinder';
import { ExplorerViewProps, View } from './View';
import { EmptyNotice } from './View/EmptyNotice';

import 'react-slidedown/lib/slidedown.css';

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
	const addNewThumbnail = useSelector(explorerStore, (s) => s.addNewThumbnail);
	const explorer = useExplorerContext();
	const layoutStore = useExplorerLayoutStore();

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
			addNewThumbnail(thumbKey);
		}
	});

	useShortcut('showPathBar', (e) => {
		e.stopPropagation();
		getExplorerLayoutStore().showPathBar = !layoutStore.showPathBar;
	});

	useShortcut('showInspector', (e) => {
		e.stopPropagation();
		if (getQuickPreviewStore().open) return;
		getExplorerStore().showInspector = !getExplorerStore().showInspector;
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
							'paddingRight': explorerStore.showInspector ? INSPECTOR_WIDTH : 0
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
						bottom={showPathBar ? PATH_BAR_HEIGHT : undefined}
					/>
				</div>
			</ExplorerContextMenu>

			{showPathBar && <ExplorerPath />}

			{explorerStore.showInspector && (
				<Inspector
					className="no-scrollbar absolute right-1.5 top-0 pb-3 pl-3 pr-1.5"
					style={{
						paddingTop: topBar.topBarHeight + 12,
						bottom: showPathBar ? PATH_BAR_HEIGHT : 0
					}}
				/>
			)}
		</>
	);
}
