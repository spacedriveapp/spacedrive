import { FC, useEffect, useState } from 'react';
import { ScrollView, Text, View } from 'react-native';
import RNFS from 'react-native-fs';
import { byteSize, Statistics, useLibraryQuery } from '@sd/client';
import useCounter from '~/hooks/useCounter';
import { tw, twStyle } from '~/lib/tailwind';

const StatItemNames: Partial<Record<keyof Statistics, string>> = {
	total_bytes_capacity: 'Total capacity',
	preview_media_bytes: 'Preview media',
	library_db_size: 'Index size',
	total_bytes_free: 'Free space'
};

const displayableStatItems = Object.keys(StatItemNames) as unknown as keyof typeof StatItemNames;

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

const StatItem: FC<{ title: string; bytes: bigint }> = ({ title, bytes }) => {
	const { value, unit } = byteSize(bytes);

	const count = useCounter({ name: title, end: value });

	return (
		<View style={tw`flex flex-col p-4`}>
			<Text style={tw`text-sm text-gray-400`}>{title}</Text>
			<View style={tw`mt-1 flex-row items-baseline`}>
				<Text style={twStyle('text-2xl font-bold tabular-nums text-white')}>{count}</Text>
				<Text style={tw`ml-1 text-sm text-gray-400`}>{unit}</Text>
			</View>
		</View>
	);
};

const OverviewStats = () => {
	const { data: libraryStatistics } = useLibraryQuery(['library.statistics'], {
		initialData: { ...EMPTY_STATISTICS }
	});

	const displayableStatItems = Object.keys(
		StatItemNames
	) as unknown as keyof typeof StatItemNames;

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

	if (!libraryStatistics) return null;

	return (
		<ScrollView horizontal showsHorizontalScrollIndicator={false}>
			{Object.entries(libraryStatistics).map(([key, bytesRaw]) => {
				if (!displayableStatItems.includes(key)) return null;
				let bytes = BigInt(bytesRaw);
				if (key === 'total_bytes_free') {
					bytes = BigInt(sizeInfo.freeSpace);
				} else if (key === 'total_bytes_capacity') {
					bytes = BigInt(sizeInfo.totalSpace);
				}
				return (
					<StatItem
						key={key}
						title={StatItemNames[key as keyof Statistics]!}
						bytes={bytes}
					/>
				);
			})}
		</ScrollView>
	);
};

export default OverviewStats;
