import { Filter, useLibraryMutation, useLibraryQuery } from '@sd/client';

import { getKey, useSearchStore } from './store';

export const useSavedSearches = () => {
	const searchStore = useSearchStore();
	const savedSearches = useLibraryQuery(['search.saved.list']);
	const createSavedSearch = useLibraryMutation(['search.saved.create']);
	const removeSavedSearch = useLibraryMutation(['search.saved.delete']);
	const searches = savedSearches.data || [];

	// const [selectedSavedSearch, setSelectedSavedSearch] = useState<number | null>(null);

	return {
		searches,
		loadSearch: (id: number) => {
			const search = searches?.find((search) => search.id === id);
			if (search) {
				// TODO
				search.filters?.forEach(({ filter_type, name, value, icon }) => {
					// const filter: Filter = {
					// 	type: filter_type,
					// 	name,
					// 	value,
					// 	icon: icon || ''
					// };
					// const key = getKey(filter);
					// searchStore.registeredFilters.set(key, filter);
					// selectFilter(filter, true);
				});
			}
		},
		removeSearch: (id: number) => {
			removeSavedSearch.mutate(id);
		},
		saveSearch: (name: string) => {
			// createSavedSearch.mutate({
			// 	name,
			// 	description: '',
			// 	icon: '',
			// 	filters: filters.map((filter) => ({
			// 		filter_type: filter.type,
			// 		name: filter.name,
			// 		value: filter.value,
			// 		icon: filter.icon || 'Folder'
			// 	}))
			// });
		}
	};
};
