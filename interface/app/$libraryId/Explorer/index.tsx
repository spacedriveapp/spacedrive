import { FolderNotchOpen } from '@phosphor-icons/react';
import { CSSProperties, type PropsWithChildren, type ReactNode } from 'react';
import { getExplorerLayoutStore, useExplorerLayoutStore, useLibrarySubscription } from '@sd/client';
import { useKeybind, useKeyMatcher } from '~/hooks';

import { TOP_BAR_HEIGHT } from '../TopBar';
import { useExplorerContext } from './Context';
import ContextMenu from './ContextMenu';
import DismissibleNotice from './DismissibleNotice';
import { Inspector, INSPECTOR_WIDTH } from './Inspector';
import ExplorerContextMenu from './ParentContextMenu';
import { useExplorerStore } from './store';
import View, { EmptyNotice, ExplorerViewProps } from './View';
import { ExplorerPath, PATH_BAR_HEIGHT } from './View/ExplorerPath';

interface Props {
	emptyNotice?: ExplorerViewProps['emptyNotice'];
	contextMenu?: () => ReactNode;
}

/**
 * This component is used in a few routes and acts as the reference demonstration of how to combine
 * all the elements of the explorer except for the context, which must be used in the parent component.
 */
export default function Explorer(props: PropsWithChildren<Props>) {
	const explorerStore = useExplorerStore();
	const explorer = useExplorerContext();
	const layoutStore = useExplorerLayoutStore();
	const metaCtrlKey = useKeyMatcher('Meta').key;

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

	useKeybind([metaCtrlKey, 'p'], (e) => {
		e.stopPropagation();
		getExplorerLayoutStore().showPathBar = !layoutStore.showPathBar;
	});

	return (
		<>
			<ExplorerContextMenu>
				<div className="flex-1 overflow-hidden">
					<div
						ref={explorer.scrollRef}
						className="custom-scroll explorer-scroll h-screen overflow-x-hidden"
						style={
							{
								'--scrollbar-margin-top': `${TOP_BAR_HEIGHT}px`,
								'--scrollbar-margin-bottom': `${
									showPathBar ? PATH_BAR_HEIGHT + 2 : 0 // TODO: Fix for web app
								}px`,
								'paddingTop': TOP_BAR_HEIGHT,
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
				</div>
			</ExplorerContextMenu>

			{showPathBar && <ExplorerPath />}

			{explorerStore.showInspector && (
				<Inspector
					className="no-scrollbar absolute right-1.5 top-0 pb-3 pl-3 pr-1.5"
					style={{
						paddingTop: TOP_BAR_HEIGHT + 12,
						bottom: showPathBar ? PATH_BAR_HEIGHT : 0
					}}
				/>
			)}
		</>
	);
}
