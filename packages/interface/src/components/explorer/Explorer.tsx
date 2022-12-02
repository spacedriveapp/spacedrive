import { ExplorerData, rspc, useCurrentLibrary } from '@sd/client';
import { useEffect, useState } from 'react';

import { useExplorerStore } from '../../util/explorerStore';
import { EncryptFileDialog } from '../dialog/EncryptFileDialog';
import { Inspector } from '../explorer/Inspector';
import ExplorerContextMenu from './ExplorerContextMenu';
import { TopBar } from './ExplorerTopBar';
import { VirtualizedList } from './VirtualizedList';

interface Props {
	data?: ExplorerData;
}

export default function Explorer(props: Props) {
	const expStore = useExplorerStore();
	const { library } = useCurrentLibrary();

	const [scrollSegments, setScrollSegments] = useState<{ [key: string]: number }>({});
	const [separateTopBar, setSeparateTopBar] = useState<boolean>(false);

	const [showEncryptDialog, setShowEncryptDialog] = useState(false);

	useEffect(() => {
		setSeparateTopBar((oldValue) => {
			const newValue = Object.values(scrollSegments).some((val) => val >= 5);

			if (newValue !== oldValue) return newValue;
			return oldValue;
		});
	}, [scrollSegments]);

	rspc.useSubscription(['jobs.newThumbnail', { library_id: library!.uuid, arg: null }], {
		onData: (cas_id) => {
			expStore.addNewThumbnail(cas_id);
		}
	});

	return (
		<>
		<EncryptFileDialog
			location_id={expStore.locationId}
			object_id={expStore.contextMenuObjectId}
			open={showEncryptDialog}
			setOpen={setShowEncryptDialog}
		/>

		<div className="relative">
			<ExplorerContextMenu setShowEncryptDialog={setShowEncryptDialog}>
				<div className="relative flex flex-col w-full">
					<TopBar showSeparator={separateTopBar} />

					<div className="relative flex flex-row w-full max-h-full app-background ">
						{props.data && (
							<VirtualizedList
								data={props.data.items || []}
								context={props.data.context}
								onScroll={(y) => {
									setScrollSegments((old) => {
										return {
											...old,
											mainList: y
										};
									});
								}}
							/>
						)}
						{expStore.showInspector && (
							<div className="flex min-w-[260px] max-w-[260px]">
								<Inspector
									onScroll={(e) => {
										const y = (e.target as HTMLElement).scrollTop;

										setScrollSegments((old) => {
											return {
												...old,
												inspector: y
											};
										});
									}}
									key={props.data?.items[expStore.selectedRowIndex]?.id}
									data={props.data?.items[expStore.selectedRowIndex]}
								/>
							</div>
						)}
					</div>
				</div>
			</ExplorerContextMenu>
		</div>
		</>
	);
}
