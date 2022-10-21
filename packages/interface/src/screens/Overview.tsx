import { PlusIcon } from '@heroicons/react/24/solid';
import {
	onLibraryChange,
	queryClient,
	useCurrentLibrary,
	useLibraryQuery,
	usePlatform
} from '@sd/client';
import { Statistics } from '@sd/client';
import { Button, Input } from '@sd/ui';
import { Dialog } from '@sd/ui';
import byteSize from 'byte-size';
import clsx from 'clsx';
import { useEffect } from 'react';
import Skeleton from 'react-loading-skeleton';
import 'react-loading-skeleton/dist/skeleton.css';
import { proxy } from 'valtio';

import useCounter from '../hooks/useCounter';

interface StatItemProps {
	title: string;
	bytes: string;
	isLoading: boolean;
}

const StatItemNames: Partial<Record<keyof Statistics, string>> = {
	total_bytes_capacity: 'Total capacity',
	preview_media_bytes: 'Preview media',
	library_db_size: 'Index size',
	total_bytes_free: 'Free space'
};

const displayableStatItems = Object.keys(StatItemNames) as unknown as keyof typeof StatItemNames;

export const state = proxy({
	lastRenderedLibraryId: undefined as string | undefined
});

onLibraryChange((newLibraryId) => {
	state.lastRenderedLibraryId = undefined;

	// TODO: Fix
	// This is bad solution to the fact that the hooks don't rerun when opening a library that is already cached.
	// This is because the count never drops back to zero as their is no loading state given the libraries data was already in the React Query cache.
	queryClient.setQueryData(
		[
			'library.getStatistics',
			{
				library_id: newLibraryId,
				arg: null
			}
		],
		{
			id: 0,
			date_captured: '',
			total_bytes_capacity: '0',
			preview_media_bytes: '0',
			library_db_size: '0',
			total_object_count: 0,
			total_bytes_free: '0',
			total_bytes_used: '0',
			total_unique_bytes: '0'
		}
	);
	queryClient.invalidateQueries(['library.getStatistics']);
});

const StatItem: React.FC<StatItemProps> = (props) => {
	const { library } = useCurrentLibrary();
	const { title, bytes = '0', isLoading } = props;

	const size = byteSize(+bytes);
	const count = useCounter({
		name: title,
		end: +size.value,
		duration: state.lastRenderedLibraryId === library?.uuid ? 0 : undefined,
		saveState: false
	});

	if (count !== 0 && count == +size.value) {
		state.lastRenderedLibraryId = library?.uuid;
	}

	useEffect(() => {
		return () => {
			if (count !== 0) state.lastRenderedLibraryId = library?.uuid;
		};
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, []);

	return (
		<div
			className={clsx(
				'flex flex-col flex-shrink-0 w-32 px-4 py-3 duration-75 transform rounded-md cursor-default ',
				!+bytes && 'hidden'
			)}
		>
			<span className="text-sm text-gray-400">{title}</span>
			<span className="text-2xl">
				{isLoading && (
					<div>
						<Skeleton enableAnimation={true} baseColor={'#21212e'} highlightColor={'#13131a'} />
					</div>
				)}
				<div
					className={clsx({
						hidden: isLoading
					})}
				>
					<span className="font-black tabular-nums">{count}</span>
					<span className="ml-1 text-[16px] text-gray-400">{size.unit}</span>
				</div>
			</span>
		</div>
	);
};

export default function OverviewScreen() {
	const platform = usePlatform();
	const { library } = useCurrentLibrary();
	const { data: overviewStats, isLoading: isStatisticsLoading } = useLibraryQuery(
		['library.getStatistics'],
		{
			initialData: {
				id: 0,
				date_captured: '',
				total_bytes_capacity: '0',
				preview_media_bytes: '0',
				library_db_size: '0',
				total_object_count: 0,
				total_bytes_free: '0',
				total_bytes_used: '0',
				total_unique_bytes: '0'
			}
		}
	);

	console.log(overviewStats);

	return (
		<div className="flex flex-col w-full h-screen overflow-x-hidden custom-scroll page-scroll app-background">
			<div data-tauri-drag-region className="flex flex-shrink-0 w-full h-5" />
			{/* PAGE */}

			<div className="flex flex-col w-full h-screen px-4">
				{/* STAT HEADER */}
				<div className="flex w-full">
					{/* STAT CONTAINER */}
					<div className="flex -mb-1 overflow-hidden">
						{Object.entries(overviewStats || []).map(([key, value]) => {
							if (!displayableStatItems.includes(key)) return null;

							return (
								<StatItem
									key={library?.uuid + ' ' + key}
									title={StatItemNames[key as keyof Statistics]!}
									bytes={value}
									isLoading={platform.demoMode === true ? false : isStatisticsLoading}
								/>
							);
						})}
					</div>

					<div className="flex-grow" />
					<div className="flex items-center h-full space-x-2">
						<div>
							{/* <Dialog
								title="Add Device"
								description="Connect a new device to your library. Either enter another device's code or copy this one."
								// ctaAction={() => {}}
								ctaLabel="Connect"
								trigger={
									<Button size="sm" variant="gray">
										<PlusIcon className="inline w-4 h-4 -mt-0.5 xl:mr-1" />
										<span className="hidden xl:inline-block">Add Device</span>
									</Button>
								}
							>
								<div className="flex flex-col mt-2 space-y-3">
									<div className="flex flex-col">
										<span className="mb-1 text-xs font-bold uppercase text-gray-450">
											This Device
										</span>
										<Input readOnly disabled value="06ffd64309b24fb09e7c2188963d0207" />
									</div>
									<div className="flex flex-col">
										<span className="mb-1 text-xs font-bold uppercase text-gray-450">
											Enter a device code
										</span>
										<Input value="" />
									</div>
								</div>
							</Dialog>*/}
						</div>
					</div>
				</div>
				<div className="flex flex-col pb-4 mt-4 space-y-4">
					{/* <Device name={`James' MacBook Pro`} size="1TB" locations={[]} type="desktop" /> */}
					{/* <Device name={`James' iPhone 12`} size="47.7GB" locations={[]} type="phone" />
					<Device name={`Spacedrive Server`} size="5GB" locations={[]} type="server" /> */}
					<Debug />
				</div>
				<div className="flex flex-shrink-0 w-full h-4" />
			</div>
		</div>
	);
}

// TODO(@Oscar): Remove this
function Debug() {
	// const org = useBridgeQuery(['normi.org']);
	// console.log(org.data);

	return null;
}
