import BrowseCategories from '~/components/browse/BrowseCategories';
import BrowseLocations from '~/components/browse/BrowseLocations';
import BrowseTags from '~/components/browse/BrowseTags';
import ScreenContainer from '~/components/layout/ScreenContainer';
import { ScrollY } from '~/types/shared';

export default function BrowseScreen({ scrollY }: ScrollY) {
	return (
		<ScreenContainer scrollY={scrollY}>
			<BrowseCategories />
			<BrowseLocations />
			<BrowseTags />
		</ScreenContainer>
	);
}
