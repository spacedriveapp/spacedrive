import { useBottomTabBarHeight } from '@react-navigation/bottom-tabs';
import { View } from 'react-native';
import { ScrollView } from 'react-native-gesture-handler';
import { useBridgeQuery, useCache, useLibraryQuery, useNodes } from '@sd/client';
import Categories from '~/components/overview/Categories';
import Cloud from '~/components/overview/Cloud';
import Devices from '~/components/overview/Devices';
import Locations from '~/components/overview/Locations';
import OverviewStats from '~/components/overview/OverviewStats';
import { twStyle } from '~/lib/tailwind';

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

export default function OverviewScreen() {
	const height = useBottomTabBarHeight();
	const { data: node } = useBridgeQuery(['nodeState']);
	const kinds = useLibraryQuery(['library.kindStatistics']);
	const stats = useLibraryQuery(['library.statistics'], {
		initialData: { ...EMPTY_STATISTICS }
	});
	const locationsQuery = useLibraryQuery(['locations.list']);
	useNodes(locationsQuery.data?.nodes);
	const locations = useCache(locationsQuery.data?.items);

	return (
		<ScrollView style={twStyle('flex-1 bg-mobile-screen', { marginBottom: height })}>
			<View style={twStyle('justify-between gap-6 py-5')}>
				<OverviewStats stats={stats} />
				<Categories kinds={kinds} />
				<Devices stats={stats} node={node} />
				<Locations locations={locations} />
				<Cloud />
			</View>
		</ScrollView>
	);
}
