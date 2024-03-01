import FiltersList from '~/components/filters/FiltersList';
import ScreenContainer from '~/components/layout/ScreenContainer';

const FiltersScreen = () => {
	return (
		<ScreenContainer tabHeight={false}>
			<FiltersList />
		</ScreenContainer>
	);
};

export default FiltersScreen;
