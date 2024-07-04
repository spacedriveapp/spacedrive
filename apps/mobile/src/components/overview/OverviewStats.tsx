import * as RNFS from '@dr.pogodin/react-native-fs';
import { AlphaRSPCError } from '@oscartbeaumont-sd/rspc-client/v2';
import { UseQueryResult } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import { Platform, Text, View } from 'react-native';
import { ClassInput } from 'twrnc/dist/esm/types';
import { humanizeSize, Statistics, StatisticsResponse, useLibraryContext } from '@sd/client';
import useCounter from '~/hooks/useCounter';
import { tw, twStyle } from '~/lib/tailwind';

import Card from '../layout/Card';

const StatItemNames: Partial<Record<keyof Statistics, string>> = {
	total_local_bytes_capacity: 'Total capacity',
	total_library_preview_media_bytes: 'Preview media',
	total_library_bytes: 'Total library size',
	library_db_size: 'Index size',
	total_local_bytes_free: 'Free space',
	total_local_bytes_used: 'Total used space'
};

interface StatItemProps {
	title: string;
	bytes: bigint;
	isLoading: boolean;
	style?: ClassInput;
}

const StatItem = ({ title, bytes, isLoading, style }: StatItemProps) => {
	const { value, unit } = humanizeSize(bytes);

	const count = useCounter({ name: title, end: value });

	return (
		<Card
			style={twStyle('flex flex-col items-center justify-center  p-2', style, {
				hidden: isLoading
			})}
		>
			<Text style={tw`text-xs font-bold text-zinc-400`}>{title}</Text>
			<View style={tw`mt-1 flex-row items-baseline`}>
				<Text style={twStyle('text-xl font-bold tabular-nums text-white')}>{count}</Text>
				<Text style={tw`ml-1 text-sm text-zinc-400`}>{unit}</Text>
			</View>
		</Card>
	);
};

interface Props {
	stats: UseQueryResult<StatisticsResponse, AlphaRSPCError>;
}

const OverviewStats = ({ stats }: Props) => {
	const { library } = useLibraryContext();

	const displayableStatItems = Object.keys(
		StatItemNames
	) as unknown as keyof typeof StatItemNames;

	// For Demo purposes as we probably wanna save this to database
	// Sets Total Capacity and Free Space of the device
	const [sizeInfo, setSizeInfo] = useState<RNFS.FSInfoResultT>({
		freeSpace: 0,
		totalSpace: 0,
		// external storage (android only) - may not be reliable
		freeSpaceEx: 0,
		totalSpaceEx: 0
	});

	useEffect(() => {
		const getFSInfo = async () => {
			return await RNFS.getFSInfo();
		};
		getFSInfo().then((size) => {
			setSizeInfo(size);
		});
	}, []);

	const renderStatItems = (isTotalStat = true) => {
		const keysToFilter = [
			'total_local_bytes_capacity',
			'total_local_bytes_used',
			'total_library_bytes'
		];
		if (!stats.data?.statistics) return null;
		return Object.entries(stats.data.statistics).map(([key, bytesRaw]) => {
			if (!displayableStatItems.includes(key)) return null;
			let bytes = BigInt(bytesRaw ?? 0);
			if (isTotalStat && !keysToFilter.includes(key)) return null;
			if (!isTotalStat && keysToFilter.includes(key)) return null;
			if (key === 'total_local_bytes_free') {
				bytes = BigInt(sizeInfo.freeSpace);
			} else if (key === 'total_local_bytes_capacity') {
				bytes = BigInt(sizeInfo.totalSpace);
			} else if (key === 'total_local_bytes_used' && Platform.OS === 'android') {
				bytes = BigInt(sizeInfo.totalSpace - sizeInfo.freeSpace);
			}
			return (
				<StatItem
					key={`${library.uuid}_${key}`}
					title={StatItemNames[key as keyof Statistics]!}
					bytes={bytes}
					isLoading={stats.isLoading}
					style={tw`w-full`}
				/>
			);
		});
	};

	return (
		<View style={tw`px-5`}>
			<Text style={tw`pb-3 text-lg font-bold text-white`}>Statistics</Text>
			<View style={tw`flex-row gap-2`}>
				<View style={tw`h-full flex-1 flex-col gap-2`}>{renderStatItems()}</View>
				<View style={tw`h-full flex-1 flex-col gap-2`}>{renderStatItems(false)}</View>
			</View>
		</View>
	);
};

export default OverviewStats;
