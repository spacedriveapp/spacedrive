import { useEffect } from 'react';
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
	const { path } = useExplorerSearchParams();

	useLibrarySubscription(['jobs.newThumbnail'], {
		onData: (cas_id) => {
			expStore.addNewThumbnail(cas_id);
		}
	});

	useEffect(() => {
		getExplorerStore().selectedRowIndex = -1;
	}, [path]);

	useKey('Space', (e) => {
		e.preventDefault();
		if (selectedRowIndex !== -1) {
			const item = props.items?.[selectedRowIndex];
			if (item) getExplorerStore().quickViewObject = item;
		}
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

				{expStore.showInspector && props.items?.[selectedRowIndex] && (
					<div className="w-[260px] shrink-0">
						<Inspector data={props.items?.[selectedRowIndex]} />
					</div>
				)}
			</div>
		</div>
	);
}
