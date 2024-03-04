import { SaveAdd } from '~/components/filters';
import FiltersList from '~/components/filters/FiltersList';
import ScreenContainer from '~/components/layout/ScreenContainer';
import { tw } from '~/lib/tailwind';

const FiltersScreen = () => {
	return (
		<>
			<ScreenContainer scrollToBottomOnChange style={tw`pb-12`} tabHeight={false}>
				<FiltersList />
			</ScreenContainer>
			<SaveAdd />
		</>
	);
};

export default FiltersScreen;
