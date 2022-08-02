import React from 'react';
import { Text, View } from 'react-native';

import { Button } from '../components/base/Button';
import Device from '../components/device/Device';
import DrawerScreenWrapper from '../components/drawer/DrawerScreenWrapper';
import OverviewStats from '../containers/OverviewStats';
import tw from '../lib/tailwind';
import { BottomNavScreenProps } from '../types/navigation';

const placeholderOverviewStats = {
	total_bytes_capacity: '8093333345230',
	preview_media_bytes: '2304387532',
	library_db_size: '83345230',
	total_file_count: '20342345',
	total_bytes_free: '89734502034',
	total_bytes_used: '8093333345230',
	total_unique_bytes: '9347397'
};

const placeholderLocations = [{}];

export default function OverviewScreen({ navigation }: BottomNavScreenProps<'Overview'>) {
	return (
		<DrawerScreenWrapper>
			<View style={tw`p-4`}>
				{/* Header */}
				<View style={tw`flex-row my-6 justify-center items-center`}>
					{/* TODO: Header with Search and a button to open drawer! */}
					<Button variant="primary" size="lg" onPress={() => navigation.openDrawer()}>
						<Text style={tw`font-bold text-white`}>Open Drawer</Text>
					</Button>
					<Button variant="primary" size="lg" onPress={() => navigation.navigate('Modal')}>
						<Text style={tw`font-bold text-white`}>Open Modal</Text>
					</Button>
				</View>
				{/* Stats */}
				<OverviewStats stats={placeholderOverviewStats} />
				{/* Devices */}
				<View style={tw`mt-4`}>
					<Device name={`James' MacBook Pro`} size="1TB" locations={[]} type="desktop" />
					<Device name={`James' iPhone 12`} size="47.7GB" locations={[]} type="phone" />
					<Device name={`Spacedrive Server`} size="5GB" locations={[]} type="server" />
				</View>
			</View>
		</DrawerScreenWrapper>
	);
}
