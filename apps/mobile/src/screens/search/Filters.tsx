import ScreenContainer from '~/components/layout/ScreenContainer';
import FiltersList from '~/components/search/filters/FiltersList';
import SaveAdd from '~/components/search/filters/SaveAdd';

const FiltersScreen = () => {
	return (
		<>
			<ScreenContainer tabHeight={false}>
				<FiltersList />
			</ScreenContainer>
			<SaveAdd />
		</>
	);
};

export default FiltersScreen;
