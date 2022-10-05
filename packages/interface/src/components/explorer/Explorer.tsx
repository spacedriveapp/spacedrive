import { ExplorerData, rspc, useCurrentLibrary, useExplorerStore } from '@sd/client';

import { Inspector } from '../explorer/Inspector';
import { TopBar } from '../layout/TopBar';
import ExplorerContextMenu from './ExplorerContextMenu';
import { VirtualizedList } from './VirtualizedList';

interface Props {
	data?: ExplorerData;
}

export default function Explorer(props: Props) {
	const expStore = useExplorerStore();
	const { library } = useCurrentLibrary();

	rspc.useSubscription(['jobs.newThumbnail', { library_id: library!.uuid, arg: null }], {
		onNext: (cas_id) => {
			expStore.addNewThumbnail(cas_id);
		}
	});

	return (
		<div className="relative">
			<ExplorerContextMenu>
				<div className="relative flex flex-col w-full bg-gray-650">
					<TopBar />

					<div className="relative flex flex-row w-full max-h-full ">
						{props.data && (
							<VirtualizedList data={props.data.items || []} context={props.data.context} />
						)}
						{expStore.showInspector && (
							<div className="flex min-w-[260px] max-w-[260px]">
								<Inspector
									key={props.data?.items[expStore.selectedRowIndex]?.id}
									data={props.data?.items[expStore.selectedRowIndex]}
								/>
							</div>
						)}
					</div>
				</div>
			</ExplorerContextMenu>
		</div>
	);
}
