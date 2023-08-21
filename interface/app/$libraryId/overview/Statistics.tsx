import clsx from 'clsx';
import { Info } from 'phosphor-react';
import Skeleton from 'react-loading-skeleton';
import 'react-loading-skeleton/dist/skeleton.css';
import { Statistics, byteSize, useLibraryContext, useLibraryQuery } from '@sd/client';
import { Tooltip } from '@sd/ui';
import { useCounter } from '~/hooks';
import { usePlatform } from '~/util/Platform';

interface StatItemProps {
	title: string;
	bytes: bigint;
	isLoading: boolean;
	info?: string;
}

const StatItemNames: Partial<Record<keyof Statistics, string>> = {
	total_bytes_capacity: 'Total capacity',
	preview_media_bytes: 'Preview media',
	library_db_size: 'Index size',
	total_bytes_free: 'Free space'
};
const StatDescriptions: Partial<Record<keyof Statistics, string>> = {
	total_bytes_capacity:
		'The total capacity of all nodes connected to the library. May show incorrect values during alpha.',
	preview_media_bytes: 'The total size of all preview media files, such as thumbnails.',
	library_db_size: 'The size of the library database.',
	total_bytes_free: 'Free space available on all nodes connected to the library.'
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

let mounted = false;

const StatItem = (props: StatItemProps) => {
	const { title, bytes, isLoading } = props;

	const size = byteSize(bytes);
	const count = useCounter({
		name: title,
		end: size.value,
		duration: mounted ? 0 : 1,
		saveState: false
	});

	return (
		<div
			className={clsx(
				'group flex w-32 shrink-0 flex-col rounded-md px-4 py-3 duration-75',
				!bytes && 'hidden'
			)}
		>
			<span className="whitespace-nowrap text-sm text-gray-400 ">
				{title}
				{props.info && (
					<Tooltip tooltipClassName="bg-black" label={props.info}>
						<Info
							weight="fill"
							className="-mt-0.5 ml-1 inline h-3 w-3 text-ink-faint opacity-0 transition-opacity group-hover:opacity-70"
						/>
					</Tooltip>
				)}
			</span>

			<span className="text-2xl">
				{isLoading && (
					<div>
						<Skeleton
							enableAnimation={true}
							baseColor={'#21212e'}
							highlightColor={'#13131a'}
						/>
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

export default () => {
	const platform = usePlatform();
	const { library } = useLibraryContext();

	const stats = useLibraryQuery(['library.statistics'], {
		initialData: { ...EMPTY_STATISTICS }
	});
	mounted = true;
	return (
		<div className="flex w-full px-5 pb-2 pt-4">
			{/* STAT CONTAINER */}
			<div className="-mb-1 flex h-20 overflow-hidden">
				{Object.entries(stats?.data || []).map(([key, value]) => {
					if (!displayableStatItems.includes(key)) return null;
					return (
						<StatItem
							key={`${library.uuid} ${key}`}
							title={StatItemNames[key as keyof Statistics]!}
							bytes={BigInt(value)}
							isLoading={stats.isLoading}
							info={StatDescriptions[key as keyof Statistics]}
						/>
					);
				})}
			</div>
		</div>
	);
};
