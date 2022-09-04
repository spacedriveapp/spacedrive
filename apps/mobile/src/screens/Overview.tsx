import React from 'react';
import { FlatList, View } from 'react-native';
import Device from '~/components/device/Device';
import VirtualizedListWrapper from '~/components/layout/VirtualizedListWrapper';
import OverviewStats from '~/containers/OverviewStats';
import tw from '~/lib/tailwind';
import { OverviewStackScreenProps } from '~/navigation/tabs/OverviewStack';

const placeholderOverviewStats = {
	id: 1,
	total_bytes_capacity: '8093333345230',
	preview_media_bytes: '2304387532',
	library_db_size: '83345230',
	total_file_count: 20342345,
	total_bytes_free: '89734502034',
	total_bytes_used: '8093333345230',
	total_unique_bytes: '9347397',
	date_captured: '2020-01-01'
};

const placeholderDevices: any = [
	{
		name: "James' iPhone 12",
		size: '47.9GB',
		locations: [],
		type: 'phone'
	},
	{
		name: "James' MacBook Pro",
		size: '1TB',
		locations: [],
		type: 'laptop'
	},
	{
		name: "James' Toaster",
		size: '1PB',
		locations: [],
		type: 'desktop'
	},
	{
		name: 'Spacedrive Server',
		size: '5GB',
		locations: [],
		type: 'server'
	}
];

export default function OverviewScreen({ navigation }: OverviewStackScreenProps<'Overview'>) {
	return (
		<VirtualizedListWrapper>
			<View style={tw`px-4 mt-4`}>
				{/* Stats */}
				<OverviewStats stats={placeholderOverviewStats} />
				{/* Devices */}
				<FlatList
					data={placeholderDevices}
					keyExtractor={(item, index) => index.toString()}
					renderItem={({ item }) => (
						<Device locations={[]} name={item.name} size={item.size} type={item.type} />
					)}
				/>
			</View>
		</VirtualizedListWrapper>
	);
}
