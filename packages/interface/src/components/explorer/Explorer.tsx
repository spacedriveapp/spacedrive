import { useCallback, useEffect, useState } from 'react';
import { ExplorerData, rspc, useCurrentLibrary } from '@sd/client';
import { useExplorerStore } from '~/hooks/useExplorerStore';
import { Inspector } from '../explorer/Inspector';
import { ExplorerContextMenu } from './ExplorerContextMenu';
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

	const onScroll = useCallback((y: number) => {
		setScrollSegments((old) => {
			return {
				...old,
				mainList: y
			};
		});
	}, []);

	return (
		<div className="relative">
			<ExplorerContextMenu>
				<div className="relative flex w-full flex-col">
					<TopBar showSeparator={separateTopBar} />

					<div className="app-background relative flex max-h-full w-full flex-row">
						{props.data && (
							<VirtualizedList
								data={props.data.items}
								context={props.data.context}
								onScroll={onScroll}
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
	);
}
