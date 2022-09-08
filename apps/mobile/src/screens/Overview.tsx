import React from 'react';
import { FlatList, View } from 'react-native';
import Device from '~/components/device/Device';
import VirtualizedListWrapper from '~/components/layout/VirtualizedListWrapper';
import OverviewStats from '~/containers/OverviewStats';
import tw from '~/lib/tailwind';
import { OverviewStackScreenProps } from '~/navigation/tabs/OverviewStack';

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
				<OverviewStats />
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
