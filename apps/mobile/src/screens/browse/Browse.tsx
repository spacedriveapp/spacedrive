import { useSharedValue } from 'react-native-reanimated';
import BrowseCategories from '~/components/browse/BrowseCategories';
import BrowseLocations from '~/components/browse/BrowseLocations';
import BrowseTags from '~/components/browse/BrowseTags';
import ScreenContainer from '~/components/layout/ScreenContainer';

export default function BrowseScreen() {
	const scrollY = useSharedValue(0);
	return (
		<ScreenContainer header={{
			scrollY: scrollY,
			showSearch: true,
			showDrawer: true,
			title: 'Browse',
		}}>
			<BrowseCategories />
			<BrowseLocations />
			<BrowseTags />
		</ScreenContainer>
	);
}
