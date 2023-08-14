import { FolderNotchOpen } from 'phosphor-react';
import { ReactNode, useEffect, useMemo, useRef, useState } from 'react';
import { ExplorerItem, useLibrarySubscription } from '@sd/client';
import { useKeyDeleteFile } from '~/hooks';
import { TOP_BAR_HEIGHT } from '../TopBar';
import { useExplorerContext } from './Context';
import ContextMenu from './ContextMenu';
import DismissibleNotice from './DismissibleNotice';
import { Inspector } from './Inspector';
import ExplorerContextMenu from './ParentContextMenu';
import View, { EmptyNotice, ExplorerViewProps } from './View';
import { useExplorerStore } from './store';
import { uniqueId, useExplorerSearchParams } from './util';

interface Props {
	items: ExplorerItem[] | null;
	onLoadMore?(): void;
	emptyNotice?: ExplorerViewProps['emptyNotice'];
	contextMenu?: (item: ExplorerItem) => ReactNode;
}

export default function Explorer(props: Props) {
	const INSPECTOR_WIDTH = 260;

	const explorerStore = useExplorerStore();

	const [{ path }] = useExplorerSearchParams();

	const scrollRef = useRef<HTMLDivElement>(null);

	const [selectedItemId, setSelectedItemId] = useState<string>();

	const selectedItem = useMemo(
		() =>
			selectedItemId
				? props.items?.find((item) => uniqueId(item) === selectedItemId)
				: undefined,
		[selectedItemId, props.items]
	);

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

	const ctx = useExplorerContext();

	useKeyDeleteFile(
		selectedItem || null,
		ctx.parent?.type === 'Location' ? ctx.parent.location.id : null
	);

	useEffect(() => setSelectedItemId(undefined), [path]);

	return (
		<>
			<ExplorerContextMenu>
				<div className="flex-1 overflow-hidden">
					<div
						ref={scrollRef}
						className="custom-scroll explorer-scroll relative h-screen overflow-x-hidden"
						style={{
							paddingTop: TOP_BAR_HEIGHT,
							paddingRight: explorerStore.showInspector ? INSPECTOR_WIDTH : 0
						}}
					>
						{props.items && props.items.length > 0 && <DismissibleNotice />}

						<View
							items={props.items}
							scrollRef={scrollRef}
							onLoadMore={props.onLoadMore}
							rowsBeforeLoadMore={5}
							selected={selectedItemId}
							onSelectedChange={setSelectedItemId}
							contextMenu={
								selectedItem ? (
									props.contextMenu ? (
										props.contextMenu(selectedItem)
									) : (
										<ContextMenu item={selectedItem} />
									)
								) : null
							}
							emptyNotice={
								props.emptyNotice || (
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
					data={selectedItem}
					className="custom-scroll inspector-scroll absolute inset-y-0 right-0 pb-4 pl-1.5 pr-1"
					style={{ paddingTop: TOP_BAR_HEIGHT + 16, width: INSPECTOR_WIDTH }}
				/>
			)}
		</>
	);
}
