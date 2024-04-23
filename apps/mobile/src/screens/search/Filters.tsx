import ScreenContainer from '~/components/layout/ScreenContainer';
import FiltersList from '~/components/search/filters/FiltersList';
import SaveAdd from '~/components/search/filters/SaveAdd';

const FiltersScreen = () => {
	return (
		<>
			<ScreenContainer bottomFadeStyle="bottom-0" tabHeight={false}>
				<FiltersList />
			</ScreenContainer>
			<SaveAdd />
		</>
	);
};

export default FiltersScreen;
