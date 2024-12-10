import { Info } from '@phosphor-icons/react';
import clsx from 'clsx';
import { useEffect, useState } from 'react';
import {
	humanizeSize,
	KindStatistic,
	Statistics,
	uint32ArrayToBigInt,
	useLibraryContext,
	useLibraryQuery,
	useLibrarySubscription
} from '@sd/client';
import { Card, Loader, Tooltip } from '@sd/ui';
import i18n from '~/app/I18n';
import { useCounter, useIsDark, useLocale } from '~/hooks';

import { FileKind, OverviewCard } from '..';
import StorageBar from '../StorageBar';

interface StatItemProps {
	title: string;
	bytes: bigint;
	isLoading: boolean;
	info?: string;
}

interface Section {
	name: string;
	value: bigint;
	color: string;
	tooltip: string;
}

function mergeKindStatistics(
	oldKindStatisticsMao: Map<number, FileKind>,
	newKindStatistics: Iterable<KindStatistic>
): Map<number, FileKind> {
	let updated = false;
	for (const stats of newKindStatistics) {
		if (uint32ArrayToBigInt(stats.count) !== 0n) {
			oldKindStatisticsMao.set(stats.kind, {
				kind: stats.kind,
				name: i18n.t(`${stats.name.toLowerCase()}`),
				count: uint32ArrayToBigInt(stats.count),
				total_bytes: uint32ArrayToBigInt(stats.total_bytes)
			});
			updated = true;
		}
	}

	// if new stats were added, return a new map due to react state update
	return updated ? new Map<number, FileKind>(oldKindStatisticsMao) : oldKindStatisticsMao;
}

const StatItem = ({ title, bytes, isLoading, info }: StatItemProps) => {
	const size = humanizeSize(bytes);
	const count = useCounter({
		name: title,
		end: size.value,
		duration: isLoading ? 0 : 1,
		saveState: false
	});
	const { t } = useLocale();

	return (
		<div
			className={clsx(
				'group/stat mt-2 flex min-w-[150px] shrink-0 flex-col font-plex duration-75',
				!bytes && 'hidden'
			)}
		>
			<span className="whitespace-nowrap text-sm font-medium text-ink-faint">
				{title}
				{info && (
					<Tooltip label={info}>
						<Info
							weight="fill"
							className="-mt-0.5 ml-1 inline size-3 text-ink-faint opacity-0 transition-opacity duration-300 group-hover/stat:opacity-70"
						/>
					</Tooltip>
				)}
			</span>
			<span className="text-2xl">
				<div className={clsx({ hidden: isLoading })}>
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
	const isDark = useIsDark();
	const { library } = useLibraryContext();
	const { t } = useLocale();
	const { data: statsData, isLoading: isStatsLoading } = useLibraryQuery(['library.statistics']);
	const { data: kindStatisticsData, isLoading: isKindStatisticsLoading } = useLibraryQuery([
		'library.kindStatistics'
	]);
	const [libraryStats, setLibraryStats] = useState<Statistics>();
	const [fileKinds, setFileKinds] = useState<Map<number, FileKind>>(new Map());
	const [loading, setLoading] = useState<boolean>(true);

	useLibrarySubscription(['library.updatedKindStatistic'], {
		onData: (data: KindStatistic) => {
			setFileKinds((kindStatisticsMap) => mergeKindStatistics(kindStatisticsMap, [data]));
		}
	});

	useEffect(() => {
		if (
			!isStatsLoading &&
			!isKindStatisticsLoading &&
			statsData &&
			statsData.statistics &&
			kindStatisticsData
		) {
			setLibraryStats(statsData.statistics);
			setFileKinds((kindStatisticsMap) =>
				mergeKindStatistics(kindStatisticsMap, Object.values(kindStatisticsData.statistics))
			);
			setLoading(false);
		}
	}, [isStatsLoading, isKindStatisticsLoading, statsData, kindStatisticsData]);

	const StatItemNames: Partial<Record<keyof Statistics, string>> = {
		total_library_bytes: t('library_bytes'),
		total_local_bytes_capacity: t('total_bytes_capacity'),
		total_local_bytes_free: t('total_bytes_free'),
		library_db_size: t('library_db_size'),
		total_library_preview_media_bytes: t('preview_media_bytes')
	};

	const StatDescriptions: Partial<Record<keyof Statistics, string>> = {
		total_library_bytes: t('library_bytes_description'),
		total_local_bytes_capacity: t('total_bytes_capacity_description'),
		total_local_bytes_free: t('total_bytes_free_description'),
		library_db_size: t('library_db_size_description'),
		total_library_preview_media_bytes: t('preview_media_bytes_description')
	};

	const displayableStatItems = Object.keys(
		StatItemNames
	) as unknown as (keyof typeof StatItemNames)[];

	// find top 5 categories by total bytes
	const aggregatedData = new Map<string, { total_bytes: bigint; color: string }>();

	for (const stats of fileKinds.values()) {
		const currentCategory = aggregatedData.get(stats.name) || { total_bytes: 0n, color: '' };
		currentCategory.total_bytes += stats.total_bytes;
		aggregatedData.set(stats.name, currentCategory);
	}

	// sort and select top 5
	const sortedCategories = [...aggregatedData.entries()].sort((a, b) => {
		if (a[1].total_bytes > b[1].total_bytes) {
			return -1;
		}
		if (a[1].total_bytes < b[1].total_bytes) {
			return 1;
		}
		return 0;
	});

	const topCategories = sortedCategories.slice(0, 5);
	const otherCategories = sortedCategories.slice(5);

	// Sum the remaining categories into "Other"
	const otherTotalBytes = otherCategories.reduce(
		(acc, [_, { total_bytes }]) => acc + total_bytes,
		0n
	);

	const colors = ['#36A3FF', '#2E84F3', '#2563EB', '#004C99', '#00274D', '#2A324B'];

	const sections: Section[] = [
		...topCategories.map(([name, { total_bytes }], index) => {
			const size = humanizeSize(total_bytes);
			return {
				name,
				value: total_bytes,
				color: colors[index % colors.length] || '#AAAAAA',
				tooltip: `${size.value} ${t(`size_${size.unit.toLowerCase()}`)}`
			};
		}),
		{
			name: t('other'),
			value: otherTotalBytes,
			color: colors[5] || '#AAAAAA',
			tooltip: `${humanizeSize(otherTotalBytes).value} ${t(`size_${humanizeSize(otherTotalBytes).unit.toLowerCase()}`)}`
		}
	];
	return (
		<>
			{loading ? (
				<div className="mt-4 flex h-full items-center justify-center">
					<div className="flex flex-col items-center justify-center gap-3">
						<Loader />
						<p className="text-ink-dull">{t('calculating_library_statistics')}</p>
					</div>
				</div>
			) : (
				<div className="flex flex-col gap-4">
					<div className="flex px-4 pt-4">
						{Object.entries(libraryStats ?? {})
							.sort(
								([a], [b]) =>
									displayableStatItems.indexOf(a as keyof typeof StatItemNames) -
									displayableStatItems.indexOf(b as keyof typeof StatItemNames)
							)
							.map(([key, value]) => {
								if (
									!displayableStatItems.includes(
										key as keyof typeof StatItemNames
									)
								)
									return null;
								return (
									<StatItem
										key={`${library.uuid} ${key}`}
										title={StatItemNames[key as keyof Statistics]!}
										bytes={BigInt(value as number)}
										isLoading={isStatsLoading}
										info={StatDescriptions[key as keyof Statistics]}
									/>
								);
							})}
					</div>
					<StorageBar sections={sections} />
				</div>
			)}
		</>
	);
};

export default LibraryStats;
