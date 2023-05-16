import { useEffect, useMemo } from 'react';
import { useKey } from 'rooks';
import { ExplorerData, useLibrarySubscription } from '@sd/client';
import { getExplorerStore, useExplorerStore } from '~/hooks/useExplorerStore';
import ExplorerContextMenu from './ContextMenu';
import { Inspector } from './Inspector';
import View from './View';
import { useExplorerSearchParams } from './util';

interface Props {
	// TODO: not using data since context isn't actually used
	// and it's not exactly compatible with search
	// data?: ExplorerData;
	items?: ExplorerData['items'];
	onLoadMore?(): void;
	hasNextPage?: boolean;
	isFetchingNextPage?: boolean;
	viewClassName?: string;
}

export default function Explorer(props: Props) {
	const { selectedRowIndex, ...expStore } = useExplorerStore();
	const [{ path }] = useExplorerSearchParams();

	useLibrarySubscription(['jobs.newThumbnail'], {
		onStarted: () => {
			console.log("Started RSPC subscription new thumbnail");
		},
		onError: (err) => {
			console.error("Error in RSPC subscription new thumbnail", err);
		},
		onData: (cas_id) => {
			console.log({ cas_id })
			expStore.addNewThumbnail(cas_id);
		}
	});

	useEffect(() => {
		getExplorerStore().selectedRowIndex = null;
	}, [path]);

	const selectedItem = useMemo(() => {
		if (selectedRowIndex === null) return null;

		return props.items?.[selectedRowIndex] ?? null;
	}, [selectedRowIndex, props.items]);

	useKey('Space', (e) => {
		e.preventDefault();

		if (selectedItem) getExplorerStore().quickViewObject = selectedItem;
	});

	return (
		<div className="flex h-screen w-full flex-col bg-app">
			<div className="flex flex-1">
				<ExplorerContextMenu>
					<div className="flex-1 overflow-hidden">
						{props.items && (
							<View
								data={props.items}
								onLoadMore={props.onLoadMore}
								hasNextPage={props.hasNextPage}
								isFetchingNextPage={props.isFetchingNextPage}
								viewClassName={props.viewClassName}
							/>
						)}
					</div>
				</ExplorerContextMenu>

				{expStore.showInspector && selectedItem !== null && (
					<div className="w-[260px] shrink-0">
						<Inspector data={selectedItem} />
					</div>
				)}
			</div>
		</div>
	);
}
