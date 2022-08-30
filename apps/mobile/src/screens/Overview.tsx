import Device from '@app/components/device/Device';
import Dialog from '@app/components/layout/Dialog';
import VirtualizedListWrapper from '@app/components/layout/VirtualizedListWrapper';
import OverviewStats from '@app/containers/OverviewStats';
import tw from '@app/lib/tailwind';
import { OverviewStackScreenProps } from '@app/navigation/tabs/OverviewStack';
import React from 'react';
import { FlatList, Text, View } from 'react-native';

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
				<Dialog
					title="Create New Library"
					description="Choose a name for your new library, you can configure this and more settings from the library settings later on."
					ctaDanger
					ctaLabel="Delete"
					ctaAction={() => console.log('wat')}
					trigger={
						<View style={tw`bg-red-200`}>
							<Text>Dialog</Text>
						</View>
					}
				/>
				{/* Stats */}
				<OverviewStats stats={placeholderOverviewStats} />
				{/* Spacing */}
				<View style={tw`mt-4`} />
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
