import { Collection, Image, Video } from '@sd/assets/icons';
import clsx from 'clsx';
import { FolderNotchOpen } from 'phosphor-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import Selecto, { SelectoProps } from 'react-selecto';
import { useKey } from 'rooks';
import { ExplorerData, useLibrarySubscription } from '@sd/client';
import {
	getExplorerStore,
	useExplorerStore,
	useSelectedExplorerItems
} from '~/hooks/useExplorerStore';
import { TOP_BAR_HEIGHT } from '../TopBar';
import ExplorerContextMenu from './ContextMenu';
import DismissibleNotice from './DismissibleNotice';
import { Inspector } from './Inspector';
import View from './View';
import { useExplorerSearchParams } from './util';

interface Props {
	// TODO: not using data since context isn't actually used
	// and it's not exactly compatible with search
	// data?: ExplorerData;
	items: ExplorerData['items'] | null;
	onLoadMore?(): void;
	hasNextPage?: boolean;
	isFetchingNextPage?: boolean;
	viewClassName?: string;
}

export default function Explorer(props: Props) {
	const { selectedRowIndex, layoutMode, ...expStore } = useExplorerStore();
	const selectedExplorerItems = useSelectedExplorerItems();
	const [{ path }] = useExplorerSearchParams();

	useLibrarySubscription(['jobs.newThumbnail'], {
		onStarted: () => {
			console.log('Started RSPC subscription new thumbnail');
		},
		onError: (err) => {
			console.error('Error in RSPC subscription new thumbnail', err);
		},
		onData: (cas_id) => {
			console.log({ cas_id });
			expStore.addNewThumbnail(cas_id);
		}
	});

	const scrollRef = useRef<HTMLDivElement>(null);
	const [selectedItems, setSelectedItems] = useState<number[]>([]);

	const selectedItem = useMemo(
		() => props.items?.filter((item) => item.item.id === selectedItems[0])[0],
		[selectedItems[0]]
	);

	useEffect(() => {
		getExplorerStore().selectedRowIndex = null;
		setSelectedItems([]);
	}, [path]);

	useKey('Space', (e) => {
		e.preventDefault();

		if (selectedItem) getExplorerStore().quickViewObject = selectedItem;
	});

	return (
		<>
			<ExplorerContextMenu>
				<div className="flex-1 overflow-hidden">
					<div
						ref={scrollRef}
						className={clsx(
							'custom-scroll explorer-scroll relative h-screen overflow-x-hidden',
							layoutMode === 'grid' && 'overflow-x-hidden',
							props.viewClassName,
							expStore.showInspector && 'pr-[260px]'
						)}
						style={{ paddingTop: TOP_BAR_HEIGHT }}
					>
						<DismissibleNotice />

						{props.items === null || (props.items && props.items.length > 0) ? (
							<View
								layout={layoutMode}
								items={props.items}
								scrollRef={scrollRef}
								onLoadMore={props.onLoadMore}
								hasNextPage={props.hasNextPage}
								isFetchingNextPage={props.isFetchingNextPage}
								selectedItems={selectedItems}
								onSelectedChange={setSelectedItems}
							/>
						) : (
							<div className="absolute left-1/2 top-1/2 flex -translate-x-1/2 -translate-y-1/2  flex-col items-center text-ink-faint">
								<FolderNotchOpen size={100} opacity={0.3} />
								<p className="mt-5 text-xs">This folder is empty</p>
							</div>
						)}
					</div>
				</div>
			</ExplorerContextMenu>

			{expStore.showInspector && (
				<Inspector
					item={selectedItem || undefined}
					className="custom-scroll inspector-scroll absolute bottom-0 right-0 top-0 w-[260px] pb-4 pl-1.5 pr-1"
					style={{ paddingTop: TOP_BAR_HEIGHT + 16 }}
				/>
			)}
		</>
	);
}
