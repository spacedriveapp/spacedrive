import BrowseLocations from '~/components/browse/BrowseLocations';
import BrowseTags from '~/components/browse/BrowseTags';
import ScreenContainer from '~/components/layout/ScreenContainer';
import { useEnableDrawer } from '~/hooks/useEnableDrawer';

export default function BrowseScreen() {
	useEnableDrawer();
	return (
		<ScreenContainer>
			{/* <BrowseCategories /> */}
			<BrowseLocations />
			<BrowseTags />
		</ScreenContainer>
	);
}
