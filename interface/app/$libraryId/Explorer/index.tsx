import { useCallback, useEffect, useRef, useState } from 'react';
import { useParams } from 'react-router';
import { ExplorerData, rspc, useLibraryContext } from '@sd/client';
import { getExplorerStore, useExplorerStore } from '~/hooks/useExplorerStore';
import { Inspector } from '../Explorer/Inspector';
import { TopBarContext } from '../PageLayout';
import TopBar, { ToolOption } from '../TopBar';
import TopBarChildren from '../TopBar/TopBarChildren';
import ExplorerContextMenu from './ContextMenu';
import View from './View';

interface Props {
	data?: ExplorerData;
	toolOptions?: ToolOption[][];
}

export default function Explorer(props: Props) {
	const expStore = useExplorerStore();
	const { library } = useLibraryContext();
	const locationId = useParams().id as string;

	rspc.useSubscription(['jobs.newThumbnail', { library_id: library.uuid, arg: null }], {
		onData: (cas_id) => {
			expStore.addNewThumbnail(cas_id);
		}
	});

	useEffect(() => {
		getExplorerStore().selectedRowIndex = -1;
	}, [locationId]);

	const ref = useRef<HTMLDivElement>(null);

	return (
		<div className="flex h-screen w-full flex-col bg-app">
			<TopBarContext.Provider value={{ topBarChildrenRef: ref }}>
				<TopBar ref={ref} />
				<TopBarChildren toolOptions={props.toolOptions} />
				<div className="flex flex-1">
					<ExplorerContextMenu>
						<div className="flex-1 overflow-hidden">
							{props.data && <View data={props.data.items} />}
						</div>
					</ExplorerContextMenu>

					{expStore.showInspector && props.data?.items[expStore.selectedRowIndex] && (
						<div className="w-[260px] shrink-0">
							<Inspector data={props.data?.items[expStore.selectedRowIndex]} />
						</div>
					)}
				</div>
			</TopBarContext.Provider>
		</div>
	);
}
