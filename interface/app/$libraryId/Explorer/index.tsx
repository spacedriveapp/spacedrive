import { FolderNotchOpen } from '@phosphor-icons/react';
import { CSSProperties, type PropsWithChildren, type ReactNode } from 'react';
import { useKeys } from 'rooks';
import { getExplorerLayoutStore, useExplorerLayoutStore, useLibrarySubscription } from '@sd/client';
import { useKeysMatcher, useOperatingSystem } from '~/hooks';

import { TOP_BAR_HEIGHT } from '../TopBar';
import { useExplorerContext } from './Context';
import ContextMenu from './ContextMenu';
import DismissibleNotice from './DismissibleNotice';
import { Inspector, INSPECTOR_WIDTH } from './Inspector';
import ExplorerContextMenu from './ParentContextMenu';
import SearchOptions from './Search';
import { useExplorerStore } from './store';
import { useKeyRevealFinder } from './useKeyRevealFinder';
import View, { EmptyNotice, ExplorerViewProps } from './View';
import { ExplorerPath, PATH_BAR_HEIGHT } from './View/ExplorerPath';

import 'react-slidedown/lib/slidedown.css';

import { useSearchStore } from './Search/store';

interface Props {
	emptyNotice?: ExplorerViewProps['emptyNotice'];
	contextMenu?: () => ReactNode;
	showFilterBar?: boolean;
}

/**
 * This component is used in a few routes and acts as the reference demonstration of how to combine
 * all the elements of the explorer except for the context, which must be used in the parent component.
 */
export default function Explorer(props: PropsWithChildren<Props>) {
	const explorerStore = useExplorerStore();
	const explorer = useExplorerContext();
	const layoutStore = useExplorerLayoutStore();

	const searchStore = useSearchStore();

	const shortcuts = useKeysMatcher(['Meta', 'Shift', 'Alt']);
	const os = useOperatingSystem();
	const hiddenFilesShortcut =
		os === 'macOS' ? [shortcuts.Meta.key, 'Shift', '.'] : [shortcuts.Meta.key, 'KeyH'];

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

	useKeys([shortcuts.Alt.key, shortcuts.Meta.key, 'KeyP'], (e) => {
		e.stopPropagation();
		getExplorerLayoutStore().showPathBar = !layoutStore.showPathBar;
	});

	useKeys(hiddenFilesShortcut, (e) => {
		e.stopPropagation();
		explorer.settingsStore.showHiddenFiles = !explorer.settingsStore.showHiddenFiles;
	});

	useKeyRevealFinder();

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

						<div className="search-options-slide sticky top-0 z-10 ">
							{searchStore.isSearching && props.showFilterBar && <SearchOptions />}
						</div>

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
