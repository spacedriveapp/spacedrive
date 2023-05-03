import { useEffect } from 'react';
import { useKey } from 'rooks';
import {
	ExplorerData,
	useBridgeSubscription,
	useLibraryContext,
	useLibrarySubscription
} from '@sd/client';
import { dialogManager } from '~/../packages/ui/src';
import { getExplorerStore, useExplorerStore } from '~/hooks/useExplorerStore';
import { Inspector } from '../Explorer/Inspector';
import { useExplorerParams } from '../location/$id';
import ExplorerContextMenu from './ContextMenu';
import DeleteDialog from './File/DeleteDialog';
import View from './View';

interface Props {
	// TODO: not using data since context isn't actually used
	// and it's not exactly compatible with search
	// data?: ExplorerData;
	items?: ExplorerData['items'];
	onLoadMore?(): void;
	hasNextPage?: boolean;
}

export default function Explorer(props: Props) {
	const { selectedRowIndex, ...expStore } = useExplorerStore();
	const { location_id, path } = useExplorerParams();

	useLibrarySubscription(['jobs.newThumbnail'], {
		onData: (cas_id) => {
			expStore.addNewThumbnail(cas_id);
		}
	});

	useEffect(() => {
		getExplorerStore().selectedRowIndex = -1;
	}, [location_id, path]);

	useKey('Space', (e) => {
		e.preventDefault();
		if (selectedRowIndex !== -1) {
			const item = props.items?.[selectedRowIndex];
			if (item) getExplorerStore().quickViewObject = item;
		}
	});

	useKey('Delete', (e) => {
		e.preventDefault();
		if (selectedRowIndex !== -1) {
			const file = props.items?.[selectedRowIndex];
			if (file && location_id)
				dialogManager.create((dp) => (
					<DeleteDialog {...dp} location_id={location_id} path_id={file.item.id} />
				));
		}
	});

	return (
		<div className="flex h-screen w-full flex-col bg-app">
			<div className="flex flex-1">
				<ExplorerContextMenu>
					<div className="flex-1 overflow-hidden">
						{props.items && <View data={props.items} onLoadMore={props.onLoadMore} />}
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
