import { useLibraryQuery } from '@sd/client';
import byteSize from 'byte-size';
import { FC, useEffect, useState } from 'react';
import { ScrollView, Text, View } from 'react-native';
import RNFS from 'react-native-fs';
import { Statistics } from '~/types/bindings';

import useCounter from '../hooks/useCounter';
import tw from '../lib/tailwind';

const StatItemNames: Partial<Record<keyof Statistics, string>> = {
	total_bytes_capacity: 'Total capacity',
	preview_media_bytes: 'Preview media',
	library_db_size: 'Index size',
	total_bytes_free: 'Free space'
};

const StatItem: FC<{ title: string; bytes: number }> = ({ title, bytes }) => {
	const { value, unit } = byteSize(+bytes);

	const count = useCounter({ name: title, end: Number(value) });

	return (
		<View style={tw`flex flex-col p-4`}>
			<Text style={tw`text-sm text-gray-400`}>{title}</Text>
			<View style={tw`flex-row items-baseline mt-1`}>
				<Text style={tw.style('text-2xl font-bold text-white tabular-nums')}>{count}</Text>
				<Text style={tw`ml-1 text-sm text-gray-400`}>{unit}</Text>
			</View>
		</View>
	);
};

const OverviewStats = () => {
	// TODO: Add loading state

	const { data: libraryStatistics } = useLibraryQuery(['library.getStatistics']);

	const displayableStatItems = Object.keys(StatItemNames) as unknown as keyof typeof StatItemNames;

	// For Demo purposes as we probably wanna save this to database
	// Sets Total Capacity and Free Space of the device
	const [sizeInfo, setSizeInfo] = useState<RNFS.FSInfoResult>({ freeSpace: 0, totalSpace: 0 });

	useEffect(() => {
		const getFSInfo = async () => {
			return await RNFS.getFSInfo();
		};
		getFSInfo().then((size) => {
			setSizeInfo(size);
		});
	}, []);

	return libraryStatistics ? (
		<ScrollView horizontal showsHorizontalScrollIndicator={false}>
			{Object.entries(libraryStatistics).map(([key, bytes]) => {
				if (!displayableStatItems.includes(key)) return null;
				if (key === 'total_bytes_free') {
					bytes = sizeInfo.freeSpace;
				} else if (key === 'total_bytes_capacity') {
					bytes = sizeInfo.totalSpace;
				}
				return <StatItem key={key} title={StatItemNames[key as keyof Statistics]!} bytes={bytes} />;
			})}
		</ScrollView>
	) : (
		<View>
			<Text style={tw`text-red-600 text-center font-bold`}>No library found...</Text>
		</View>
	);
};

export default OverviewStats;
