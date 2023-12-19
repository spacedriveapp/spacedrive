import { Info } from '@phosphor-icons/react';
import clsx from 'clsx';
import { useEffect, useState } from 'react';
import { byteSize, Statistics, useLibraryContext, useLibraryQuery } from '@sd/client';
import { Card, Tooltip } from '@sd/ui';
import { useCounter } from '~/hooks';

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
	total_bytes_free: 'Free space',
	total_bytes_used: 'Total used space'
};
const StatDescriptions: Partial<Record<keyof Statistics, string>> = {
	total_bytes_capacity:
		'The total capacity of all nodes connected to the library. May show incorrect values during alpha.',
	preview_media_bytes: 'The total size of all preview media files, such as thumbnails.',
	library_db_size: 'The size of the library database.',
	total_bytes_free: 'Free space available on all nodes connected to the library.',
	total_bytes_used: 'Total space used on all nodes connected to the library.'
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

	// This is important to the counter working.
	// On first render of the counter this will potentially be `false` which means the counter should the count up.
	// but in a `useEffect` `mounted` will be set to `true` so that subsequent renders of the counter will not run the count up.
	// The acts as a cache of the value of `mounted` on the first render of this `StateItem`.
	const [isMounted] = useState(mounted);

	const size = byteSize(bytes);
	const count = useCounter({
		name: title,
		end: size.value,
		duration: isMounted ? 0 : 1,
		saveState: false
	});

	return (
		<div className={clsx('group flex w-32 shrink-0 flex-col duration-75', !bytes && 'hidden')}>
			<span className="whitespace-nowrap text-sm font-medium text-ink-faint">
				{title}
				{props.info && (
					<Tooltip label={props.info}>
						<Info
							weight="fill"
							className="-mt-0.5 ml-1 inline h-3 w-3 text-ink-faint opacity-0 transition-opacity group-hover:opacity-70"
						/>
					</Tooltip>
				)}
			</span>

			<span className="text-2xl">
				<div
					className={clsx({
						hidden: isLoading
					})}
				>
					<span className="font-black tabular-nums">{count}</span>
					<span className="ml-1 text-[16px] font-medium text-ink-faint">{size.unit}</span>
				</div>
			</span>
		</div>
	);
};

const LibraryStatistics = () => {
	const { library } = useLibraryContext();

	const stats = useLibraryQuery(['library.statistics'], {
		initialData: { ...EMPTY_STATISTICS }
	});

	useEffect(() => {
		if (!stats.isLoading) mounted = true;
	});

	return (
		<div className="flex w-full">
			<div className="flex gap-3 overflow-hidden">
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

export default LibraryStatistics;
