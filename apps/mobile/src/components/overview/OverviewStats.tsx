import { AlphaRSPCError } from '@oscartbeaumont-sd/rspc-client/v2';
import { UseQueryResult } from '@tanstack/react-query';
import { Text, View } from 'react-native';
import { ClassInput } from 'twrnc/dist/esm/types';
import { byteSize, Statistics, StatisticsResponse, useLibraryContext } from '@sd/client';
import useCounter from '~/hooks/useCounter';
import { tw, twStyle } from '~/lib/tailwind';

const StatItemNames: Partial<Record<keyof Statistics, string>> = {
	total_bytes_capacity: 'Total capacity',
	preview_media_bytes: 'Preview media',
	library_db_size: 'Index size',
	total_bytes_free: 'Free space',
	total_bytes_used: 'Total used space'
};

interface StatItemProps {
	title: string;
	bytes: bigint;
	isLoading: boolean;
	style?: ClassInput;
}

const StatItem = ({ title, bytes, isLoading, style }: StatItemProps) => {
	const { value, unit } = byteSize(bytes);

	const count = useCounter({ name: title, end: value });

	return (
		<View
			style={twStyle(
				'flex flex-col items-center justify-center rounded-md border border-sidebar-line/50 bg-sidebar-box p-2',
				style,
				{
					hidden: isLoading
				}
			)}
		>
			<Text style={tw`text-sm font-bold text-gray-400`}>{title}</Text>
			<View style={tw`flex-row items-baseline mt-1`}>
				<Text style={twStyle('text-xl font-bold tabular-nums text-white')}>{count}</Text>
				<Text style={tw`ml-1 text-sm text-gray-400`}>{unit}</Text>
			</View>
		</View>
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

	const renderStatItems = (isTotalStat = true) => {
		return Object.entries(stats?.data?.statistics || []).map(([key, value]) => {
			if (!displayableStatItems.includes(key)) return null;
			if (isTotalStat && !['total_bytes_capacity', 'total_bytes_used'].includes(key))
				return null;
			if (!isTotalStat && ['total_bytes_capacity', 'total_bytes_used'].includes(key))
				return null;

			return (
				<StatItem
					key={`${library.uuid} ${key}`}
					title={StatItemNames[key as keyof Statistics]!}
					bytes={BigInt(value)}
					isLoading={stats.isLoading}
					style={tw`${isTotalStat ? 'h-[101px] w-full' : 'w-full'} flex-1`}
				/>
			);
		});
	};

	return (
		<View style={tw`px-7`}>
			<Text style={tw`pb-5 text-lg font-bold text-white`}>Statistics</Text>
			<View style={tw`h-[250px] w-full flex-row justify-between gap-2`}>
				<View style={tw`h-full w-[49%] flex-col justify-between gap-2`}>
					{renderStatItems()}
				</View>
				<View style={tw`h-full w-[49%] flex-col justify-between gap-2`}>
					{renderStatItems(false)}
				</View>
			</View>
		</View>
	);
};

export default OverviewStats;
