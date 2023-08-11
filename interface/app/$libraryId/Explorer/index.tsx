import { FolderNotchOpen } from 'phosphor-react';
import { PropsWithChildren, ReactNode } from 'react';
import { useLibrarySubscription } from '@sd/client';
import { TOP_BAR_HEIGHT } from '../TopBar';
import { useExplorerContext } from './Context';
import ContextMenu from './ContextMenu';
import DismissibleNotice from './DismissibleNotice';
import { Inspector } from './Inspector';
import ExplorerContextMenu from './ParentContextMenu';
import View, { EmptyNotice, ExplorerViewProps } from './View';
import { useExplorerStore } from './store';

interface Props {
	emptyNotice?: ExplorerViewProps['emptyNotice'];
	contextMenu?: () => ReactNode;
}

const INSPECTOR_WIDTH = 260;

/**
 * This component is used in a few routes and acts as the reference demonstration of how to combine
 * all the elements of the explorer except for the context, which must be used in the parent component.
 */
export default function Explorer(props: PropsWithChildren<Props>) {
	const explorerStore = useExplorerStore();
	const explorer = useExplorerContext();

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

	return (
		<>
			<ExplorerContextMenu>
				<div className="flex-1 overflow-hidden">
					<div
						ref={explorer.scrollRef}
						className="custom-scroll explorer-scroll relative h-screen overflow-x-hidden"
						style={{
							paddingTop: TOP_BAR_HEIGHT,
							paddingRight: explorerStore.showInspector ? INSPECTOR_WIDTH : 0
						}}
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
						/>
					</div>
				</div>
			</ExplorerContextMenu>

			{explorerStore.showInspector && (
				<Inspector
					className="custom-scroll inspector-scroll absolute inset-y-0 right-0 pb-4 pl-1.5 pr-1"
					style={{ paddingTop: TOP_BAR_HEIGHT + 16, width: INSPECTOR_WIDTH }}
				/>
			)}
		</>
	);
}
