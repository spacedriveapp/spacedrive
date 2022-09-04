import {
	rspc,
	useExplorerStore,
	useLibraryMutation,
	useLibraryQuery,
	useLibraryStore
} from '@sd/client';
import { ExplorerData } from '@sd/core';
import {
	ArrowBendUpRight,
	LockSimple,
	Package,
	Plus,
	Share,
	TagSimple,
	Trash,
	TrashSimple
} from 'phosphor-react';
import React, { memo, useLayoutEffect, useMemo, useRef, useState } from 'react';

import { Inspector } from '../explorer/Inspector';
import { WithContextMenu } from '../layout/MenuOverlay';
import { TopBar } from '../layout/TopBar';
import ExplorerContextMenu from './ExplorerContextMenu';
import { VirtualizedList } from './VirtualizedList';

interface Props {
	data: ExplorerData;
}

export default function Explorer(props: Props) {
	const addNewThumbnail = useExplorerStore((store) => store.addNewThumbnail);

	const currentLibraryUuid = useLibraryStore((store) => store.currentLibraryUuid);

	rspc.useSubscription(['jobs.newThumbnail', { library_id: currentLibraryUuid!, arg: null }], {
		onNext: (cas_id) => {
			addNewThumbnail(cas_id);
		}
	});

	const { selectedRowIndex, showInspector } = useExplorerStore((store) => ({
		selectedRowIndex: store.selectedRowIndex,
		showInspector: store.showInspector
	}));

	return (
		<div className="relative">
			<ExplorerContextMenu>
				<div className="relative flex flex-col w-full bg-gray-650">
					<TopBar />
					<div className="relative flex flex-row w-full max-h-full">
						<VirtualizedList data={props.data?.items || []} context={props.data.context} />
						{showInspector && (
							<div className="min-w-[260px] max-w-[260px]">
								{props.data.items[selectedRowIndex]?.id && (
									<Inspector
										key={props.data.items[selectedRowIndex].id}
										data={props.data.items[selectedRowIndex]}
									/>
								)}
							</div>
						)}
					</div>
				</div>
			</ExplorerContextMenu>
		</div>
	);
}
