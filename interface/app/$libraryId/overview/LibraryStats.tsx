import { Info } from '@phosphor-icons/react';
import clsx from 'clsx';
import { useEffect, useState } from 'react';
import { humanizeSize, Statistics, useLibraryContext, useLibraryQuery } from '@sd/client';
import { Tooltip } from '@sd/ui';
import { useCounter, useLocale } from '~/hooks';

interface StatItemProps {
	title: string;
	bytes: bigint;
	isLoading: boolean;
	info?: string;
}

let mounted = false;

const StatItem = (props: StatItemProps) => {
	const { title, bytes, isLoading } = props;

	// This is important to the counter working.
	// On first render of the counter this will potentially be `false` which means the counter should the count up.
	// but in a `useEffect` `mounted` will be set to `true` so that subsequent renders of the counter will not run the count up.
	// The acts as a cache of the value of `mounted` on the first render of this `StateItem`.
	const [isMounted] = useState(mounted);

	const size = humanizeSize(bytes);
	const count = useCounter({
		name: title,
		end: size.value,
		duration: isMounted ? 0 : 1,
		saveState: false
	});

	const { t } = useLocale();

	return (
		<div
			className={clsx(
				'group/stat flex w-32 shrink-0 flex-col duration-75',
				!bytes && 'hidden'
			)}
		>
			<span className="whitespace-nowrap text-sm font-medium text-ink-faint">
				{title}
				{props.info && (
					<Tooltip label={props.info}>
						<Info
							weight="fill"
							className="-mt-0.5 ml-1 inline size-3 text-ink-faint opacity-0 transition-opacity group-hover/stat:opacity-70"
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
					<span className="ml-1 text-[16px] font-medium text-ink-faint">
						{t(`size_${size.unit.toLowerCase()}`)}
					</span>
				</div>
			</span>
		</div>
	);
};

const LibraryStats = () => {
	const { library } = useLibraryContext();

	const stats = useLibraryQuery(['library.statistics']);

	useEffect(() => {
		if (!stats.isLoading) mounted = true;
	});

	const { t } = useLocale();

	const StatItemNames: Partial<Record<keyof Statistics, string>> = {
		total_library_bytes: t('library_bytes'),
		library_db_size: t('library_db_size'),
		total_local_bytes_capacity: t('total_bytes_capacity'),
		total_library_preview_media_bytes: t('preview_media_bytes'),
		total_local_bytes_free: t('total_bytes_free'),
		total_local_bytes_used: t('total_bytes_used')
	};

	const StatDescriptions: Partial<Record<keyof Statistics, string>> = {
		total_local_bytes_capacity: t('total_bytes_capacity_description'),
		total_library_preview_media_bytes: t('preview_media_bytes_description'),
		total_library_bytes: t('library_bytes_description'),
		library_db_size: t('library_db_size_description'),
		total_local_bytes_free: t('total_bytes_free_description'),
		total_local_bytes_used: t('total_bytes_used_description')
	};

	const displayableStatItems = Object.keys(
		StatItemNames
	) as unknown as keyof typeof StatItemNames;
	return (
		<div className="flex w-full">
			<div className="flex gap-3 overflow-hidden">
				{Object.entries(stats?.data?.statistics || [])
					// sort the stats by the order of the displayableStatItems
					.sort(
						([a], [b]) =>
							displayableStatItems.indexOf(a) - displayableStatItems.indexOf(b)
					)
					.map(([key, value]) => {
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

export default LibraryStats;
