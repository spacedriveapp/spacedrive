import byteSize from 'byte-size';
import clsx from 'clsx';
import Skeleton from 'react-loading-skeleton';
import 'react-loading-skeleton/dist/skeleton.css';
import { Statistics, useLibraryContext, useLibraryQuery } from '@sd/client';
import { useCounter } from '~/hooks';
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

let mounted = false;

const StatItem = (props: StatItemProps) => {
	const { title, bytes = BigInt('0'), isLoading } = props;

	const size = byteSize(Number(bytes)); // TODO: This BigInt to Number conversion will truncate the number if the number is too large. `byteSize` doesn't support BigInt so we are gonna need to come up with a longer term solution at some point.
	const count = useCounter({
		name: title,
		end: +size.value,
		duration: mounted ? 0 : 1,
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
		<div className="flex w-full px-5 pt-4">
			{/* STAT CONTAINER */}
			<div className="-mb-1 flex h-20 overflow-hidden">
				{Object.entries(stats?.data || []).map(([key, value]) => {
					if (!displayableStatItems.includes(key)) return null;
					return (
						<StatItem
							key={`${library.uuid} ${key}`}
							title={StatItemNames[key as keyof Statistics]!}
							bytes={BigInt(value)}
							isLoading={platform.demoMode ? false : stats.isLoading}
						/>
					);
				})}
			</div>
		</div>
	);
};
