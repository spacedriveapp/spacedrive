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
import Skeleton from 'react-loading-skeleton';
import 'react-loading-skeleton/dist/skeleton.css';
import { Statistics, useLibraryQuery } from '@sd/client';
import { Card } from '@sd/ui';
import useCounter from '~/hooks/useCounter';
import { useLibraryId } from '~/util';
import { usePlatform } from '~/util/Platform';

interface StatItemProps {
	title: string;
	bytes: bigint;
	isLoading: boolean;
}

const StatItemNames: Partial<Record<keyof Statistics, string>> = {
	total_bytes_capacity: 'Total capacity',
	preview_media_bytes: 'Preview media',
	library_db_size: 'Index size',
	total_bytes_free: 'Free space'
};

const EMPTY_STATISTICS = {
	id: 0,
	date_captured: '',
	total_bytes_capacity: '0',
	preview_media_bytes: '0',
	library_db_size: '0',
	total_object_count: 0,
	total_bytes_free: '0',
	total_bytes_used: '0',
	total_unique_bytes: '0'
};

const displayableStatItems = Object.keys(StatItemNames) as unknown as keyof typeof StatItemNames;

let overviewMounted = false;

const StatItem = (props: StatItemProps) => {
	const { title, bytes = BigInt('0'), isLoading } = props;

	const size = byteSize(Number(bytes)); // TODO: This BigInt to Number conversion will truncate the number if the number is too large. `byteSize` doesn't support BigInt so we are gonna need to come up with a longer term solution at some point.
	const count = useCounter({
		name: title,
		end: +size.value,
		duration: overviewMounted ? 0 : 1,
		saveState: false
	});

	return (
		<div
			className={clsx(
				'flex w-32 shrink-0 cursor-default flex-col rounded-md px-4 py-3 duration-75',
				!bytes && 'hidden'
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
	const libraryId = useLibraryId();

	const stats = useLibraryQuery(['library.getStatistics'], {
		initialData: { ...EMPTY_STATISTICS }
	});

	overviewMounted = true;

	return (
		<div className="custom-scroll page-scroll app-background flex h-screen w-full flex-col overflow-x-hidden">
			<div data-tauri-drag-region className="flex h-5 w-full shrink-0" />
			{/* PAGE */}

			<div className="flex h-screen w-full flex-col px-4">
				{/* STAT HEADER */}
				<div className="flex w-full">
					{/* STAT CONTAINER */}
					<div className="-mb-1 flex h-20 overflow-hidden">
						{Object.entries(stats?.data || []).map(([key, value]) => {
							if (!displayableStatItems.includes(key)) return null;
							return (
								<StatItem
									key={`${libraryId} ${key}`}
									title={StatItemNames[key as keyof Statistics]!}
									bytes={BigInt(value)}
									isLoading={platform.demoMode ? false : stats.isLoading}
								/>
							);
						})}
					</div>
					<div className="grow" />
				</div>
				<div className="mt-4 grid grid-cols-5 gap-3 pb-4">
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
				</div>
				<Card className="text-ink-dull">
					<b>Note: </b>&nbsp; This is a pre-alpha build of Spacedrive, many features are yet to be
					functional.
				</Card>
				<div className="flex h-4 w-full shrink-0" />
			</div>
		</div>
	);
}

interface CategoryButtonProps {
	category: string;
	icon: any;
}

function CategoryButton({ category, icon: Icon }: CategoryButtonProps) {
	return (
		<Card className="items-center !px-3">
			<Icon weight="fill" className="text-ink-dull mr-3 h-6 w-6 opacity-20" />
			<div>
				<h2 className="text-sm font-medium">{category}</h2>
				<p className="text-ink-faint text-xs">23,324 items</p>
			</div>
		</Card>
	);
}
