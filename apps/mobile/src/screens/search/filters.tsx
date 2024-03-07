import { useEffect } from 'react';
import { FiltersList, SaveAdd } from '~/components/filters';
import ScreenContainer from '~/components/layout/ScreenContainer';
import { getSearchStore, useSearchStore } from '~/stores/searchStore';

const FiltersScreen = () => {
	const searchStore = useSearchStore();
	// enable action buttons if any filter value is present
	useEffect(() => {
		const hasNonEmptyFilter = Object.values(searchStore.filters)
			.flat()
			.some((v) => v !== '' && v !== false);
		getSearchStore().disableActionButtons = !hasNonEmptyFilter;
	}, [searchStore.filters]);

	// Reset filters when the screen is unmounted
	// useEffect(() => {
	// 	const resetFilters = () => {
	// 		searchStore.resetFilters();
	// 	};
	// 	return resetFilters;
	// }, []);

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
