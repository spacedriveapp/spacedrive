import BrowseLocations from '~/components/browse/BrowseLocations';
import BrowseTags from '~/components/browse/BrowseTags';
import ScreenContainer from '~/components/layout/ScreenContainer';

export default function BrowseScreen() {
	return (
		<ScreenContainer>
			{/* <BrowseCategories /> */}
			<BrowseLocations />
			<BrowseTags />
		</ScreenContainer>
	);
}
