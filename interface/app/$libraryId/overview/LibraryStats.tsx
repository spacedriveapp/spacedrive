import { Info } from '@phosphor-icons/react';
import clsx from 'clsx';
import { useEffect, useState } from 'react';
import { humanizeSize, Statistics, useLibraryContext, useLibraryQuery } from '@sd/client';
import { Card, Tooltip } from '@sd/ui';
import { useCounter, useIsDark, useLocale } from '~/hooks';

import StorageBar from './StorageBar';

interface StatItemProps {
	title: string;
	bytes: bigint;
	isLoading: boolean;
	info?: string;
}

interface Section {
	name: string;
	value: number;
	color: string;
	tooltip: string;
}

let mounted = false;

const StatItem = (props: StatItemProps) => {
	const { title, bytes, isLoading } = props;

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
				'group/stat flex w-36 shrink-0 flex-col duration-75',
				!bytes && 'hidden'
			)}
		>
			<span className="whitespace-nowrap text-sm font-medium text-ink-faint">
				{title}
				{props.info && (
					<Tooltip label={props.info}>
						<Info
							weight="fill"
							className="-mt-0.5 ml-1 inline size-3 text-ink-faint opacity-0 transition-opacity duration-300 group-hover/stat:opacity-70"
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
	const isDark = useIsDark();
	const { library } = useLibraryContext();
	const stats = useLibraryQuery(['library.statistics']);
	const storageBarData = useLibraryQuery(['library.kindStatistics']).data?.statistics;
	const { t } = useLocale();

	useEffect(() => {
		if (!stats.isLoading) mounted = true;
	}, [stats.isLoading]);

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
	) as unknown as keyof typeof StatItemNames;

	if (!stats.data || !stats.data.statistics) {
		return <div>Loading...</div>;
	}

	const { statistics } = stats.data;
	const totalSpace = Number(statistics.total_local_bytes_capacity);
	const totalUsedSpace = Number(statistics.total_local_bytes_used);

	// Define the major categories and aggregate the "Other" category
	const majorCategories = ['Document', 'Text', 'Image', 'Video'];
	const aggregatedData = (storageBarData ?? []).reduce(
		(acc, curr) => {
			const category = majorCategories.includes(curr.name) ? curr.name : 'Other';
			if (!acc[category]) {
				acc[category] = { total_bytes: 0 };
			}
			acc[category]!.total_bytes += curr.total_bytes[1];
			return acc;
		},
		{} as Record<string, { total_bytes: number }>
	);

	// Calculate the used space and determine the System Data
	const usedSpace = Object.values(aggregatedData).reduce(
		(acc, curr) => acc + curr.total_bytes,
		0
	);
	const systemDataBytes = totalUsedSpace - usedSpace;

	if (!aggregatedData['Other']) {
		aggregatedData['Other'] = { total_bytes: 0 };
	}

	const sections: Section[] = Object.entries(aggregatedData).map(([name, data], index) => {
		const colors = [
			'#3A7ECC', // Slightly Darker Blue 400
			'#AAAAAA', // Gray
			'#004C99', // Tailwind Blue 700
			'#2563EB', // Tailwind Blue 500
			'#00274D' // Dark Navy Blue,
		];

		const color = colors[index % colors.length] || '#8F8F8F'; // Use a default color if colors array is empty
		return {
			name,
			value: data.total_bytes,
			color,
			tooltip: `${name}`
		};
	});

	// Add System Data section
	sections.push({
		name: 'System Data',
		value: systemDataBytes,
		color: '#2F3038', // Gray for System Data
		tooltip: 'System data that exists outside of your Spacedrive library'
	});

	return (
		<Card className="flex h-[220px] w-[750px] shrink-0 flex-col bg-app-box/50">
			<div className="mb-1 flex overflow-hidden p-4">
				{Object.entries(statistics)
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
								bytes={BigInt(value as number)}
								isLoading={stats.isLoading}
								info={StatDescriptions[key as keyof Statistics]}
							/>
						);
					})}
			</div>
			<div>
				<StorageBar sections={sections} totalSpace={totalSpace} />
			</div>
		</Card>
	);
};

export default LibraryStats;
