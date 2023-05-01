import { useEffect } from 'react';
import { useParams } from 'react-router';
import { useKey } from 'rooks';
import { ExplorerData, useBridgeSubscription, useLibraryContext } from '@sd/client';
import { getExplorerStore, useExplorerStore } from '~/hooks/useExplorerStore';
import { Inspector } from '../Explorer/Inspector';
import { useExplorerParams } from '../location/$id';
import ExplorerContextMenu from './ContextMenu';
import View from './View';

interface Props {
	// TODO: not using data since context isn't actually used
	// and it's not exactly compatible with search
	// data?: ExplorerData;
	items?: ExplorerData['items'];
}

export default function Explorer(props: Props) {
	const { selectedRowIndex, ...expStore } = useExplorerStore();
	const { library } = useLibraryContext();
	const { location_id, path } = useExplorerParams();

	useBridgeSubscription(['jobs.newThumbnail', { library_id: library.uuid, arg: null }], {
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

	return (
		<div className="flex h-screen w-full flex-col bg-app">
			<div className="flex flex-1">
				<ExplorerContextMenu>
					<div className="flex-1 overflow-hidden">
						{props.items && <View data={props.items} />}
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
