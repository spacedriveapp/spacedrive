import { useBridgeQuery, useLibraryQuery } from '@sd/client';
import ScreenContainer from '~/components/layout/ScreenContainer';
import Categories from '~/components/overview/Categories';
import Cloud from '~/components/overview/Cloud';
import Devices from '~/components/overview/Devices';
import Locations from '~/components/overview/Locations';
import OverviewStats from '~/components/overview/OverviewStats';
import { useEnableDrawer } from '~/hooks/useEnableDrawer';

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
	const { data: node } = useBridgeQuery(['nodeState']);
	const stats = useLibraryQuery(['library.statistics'], {
		initialData: { ...EMPTY_STATISTICS }
	});

	// Running the query here so the data is already available for settings screen
	useLibraryQuery(['sync.enabled']);
	useEnableDrawer();

	return (
		<ScreenContainer>
			<OverviewStats stats={stats} />
			<Categories />
			<Devices stats={stats} node={node} />
			<Locations />
			<Cloud />
		</ScreenContainer>
	);
}
