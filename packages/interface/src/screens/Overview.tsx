import { useQueryClient } from '@tanstack/react-query';
import byteSize from 'byte-size';
import clsx from 'clsx';
import {
	AppWindow,
	Camera,
	CloudArrowDown,
	FileText,
	FrameCorners,
	Heart,
	Image,
	MusicNote,
	Wrench
} from 'phosphor-react';
import { useEffect } from 'react';
import Skeleton from 'react-loading-skeleton';
import 'react-loading-skeleton/dist/skeleton.css';
import { proxy } from 'valtio';
import { Statistics, onLibraryChange, useCurrentLibrary, useLibraryQuery } from '@sd/client';
import { Card } from '@sd/ui';
import useCounter from '~/hooks/useCounter';
import { usePlatform } from '~/util/Platform';

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

	const queryClient = useQueryClient();
	useEffect(() => {
		// return makes sure this is unsubscribed when the component is unmounted
		return onLibraryChange((newLibraryId) => {
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
	});

	return (
		<div className="flex flex-col w-full h-screen overflow-x-hidden custom-scroll page-scroll app-background">
			<div data-tauri-drag-region className="flex flex-shrink-0 w-full h-5" />
			{/* PAGE */}

			<div className="flex flex-col w-full h-screen px-4">
				{/* STAT HEADER */}
				<div className="flex w-full">
					{/* STAT CONTAINER */}
					<div className="flex h-20 -mb-1 overflow-hidden">
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
				</div>
				<div className="grid grid-cols-5 gap-3 pb-4 mt-4">
					<CategoryButton icon={Heart} category="Favorites" />
					<CategoryButton icon={FileText} category="Documents" />
					<CategoryButton icon={Camera} category="Movies" />
					<CategoryButton icon={FrameCorners} category="Screenshots" />
					<CategoryButton icon={AppWindow} category="Applications" />
					<CategoryButton icon={Wrench} category="Projects" />
					<CategoryButton icon={CloudArrowDown} category="Downloads" />
					<CategoryButton icon={MusicNote} category="Music" />
					<CategoryButton icon={Image} category="Albums" />
					<CategoryButton icon={Heart} category="Favorites" />
					<Debug />
				</div>
				<Card className="text-ink-dull">
					<b>Note: </b>&nbsp; This is a pre-alpha build of Spacedrive, many features are yet to be
					functional.
				</Card>
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

interface CategoryButtonProps {
	category: string;
	icon: any;
}

function CategoryButton({ category, icon: Icon }: CategoryButtonProps) {
	return (
		<Card className="!px-3 items-center">
			<Icon weight="fill" className="w-6 h-6 mr-3 text-ink-dull opacity-20" />
			<div>
				<h2 className="text-sm font-medium">{category}</h2>
				<p className="text-xs text-ink-faint">23,324 items</p>
			</div>
		</Card>
	);
}
