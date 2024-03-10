import FiltersList from '~/components/filters/FiltersList';
import SaveAdd from '~/components/filters/SaveAdd';
import ScreenContainer from '~/components/layout/ScreenContainer';

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
