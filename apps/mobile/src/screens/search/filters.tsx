import { useEffect } from 'react';
import { View } from 'react-native';
import { SaveAdd } from '~/components/filters';
import FiltersList from '~/components/filters/FiltersList';
import ScreenContainer from '~/components/layout/ScreenContainer';
import { tw } from '~/lib/tailwind';
import { getSearchStore, useSearchStore } from '~/stores/searchStore';

const FiltersScreen = () => {
	const searchStore = useSearchStore();

	// Show action buttons if any filter value is present
	useEffect(() => {
		const hasNonEmptyFilter = Object.values(searchStore.filters).some((filterValues) =>
			filterValues.some((value) => value !== '')
		);

		getSearchStore().showActionButtons = hasNonEmptyFilter;
	}, [searchStore.filters]);

	// Reset filters when the screen is unmounted
	useEffect(() => {
		const resetFilters = () => {
			getSearchStore().resetFilters();
		};
		return resetFilters;
	}, []);

	return (
		<View style={tw`flex-1 bg-mobile-screen`}>
			<ScreenContainer style={tw`pb-12`} tabHeight={false}>
				<FiltersList />
			</ScreenContainer>
			<View style={tw`bg-mobile-screen`}>
				<SaveAdd />
			</View>
		</View>
	);
};

export default FiltersScreen;
