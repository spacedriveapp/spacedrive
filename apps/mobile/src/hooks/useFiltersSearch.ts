import { SearchFilterArgs } from '@sd/client';
import { useEffect, useMemo } from 'react';
import { Filters, SearchFilters, getSearchStore, useSearchStore } from '~/stores/searchStore';

/**
 * This hook merges the selected filters from Filters page in order
 * to make query calls for saved searches and setups filters for the search
 * the data structure has been designed to match the desktop app
 */

export function useFiltersSearch() {
	const searchStore = useSearchStore();;


	const filterFactory = (key: SearchFilters, value: Filters[keyof Filters])  => {

		//hidden is the only boolean filter - so we can return it directly
		//Rest of the filters are arrays, so we map them to the correct format
		const filterValue = Array.isArray(value) ? value.map((v: any) => {
			return v.id ? v.id : v;
		}) : value;

		//switch case for each filter
		//This makes it easier to add new filters in the future and setup
		//the correct object of each filter accordingly and easily

			switch (key) {
				case 'locations':
					return { filePath: { locations: { in: filterValue } } };
				case 'name':
					return Array.isArray(filterValue) && filterValue.map((v: string) => {
						return { filePath: { [key]: { contains: v } } };
					})
				case 'hidden':
					return { filePath: { hidden: filterValue } };
				case 'extension':
					return Array.isArray(filterValue) && filterValue.map((v: string) => {
						return { filePath: { [key]: { in: [v] } } };
					})
				case 'tags':
					return { object: { tags: { in: filterValue } } };
				case 'kind':
					return { object: { kind: { in: filterValue } } };
				default:
					return {};
			}
	}


	const mergedFilters = useMemo(() => {

		    const filters = [] as SearchFilterArgs[];

			for (const key in searchStore.filters) {

				const filterKey = key as SearchFilters;
				//due to an issue with Valtio and Hermes Engine - need to do getSearchStore()
				//https://github.com/pmndrs/valtio/issues/765
				const filterValue = getSearchStore().filters[filterKey];

				// no need to add empty filters
				if (Array.isArray(filterValue)) {
					const realValues = filterValue.filter((v) => v !== '');
					if (realValues.length === 0) {
						continue;
					}
				}

				// create the filter object
				const filter = filterFactory(filterKey, filterValue);

				// add the filter to the mergedFilters
				filters.push(filter as SearchFilterArgs);

			}

			// makes sure the array is not 2D
			return filters.flat();

	}, [searchStore.filters]);


	useEffect(() => {
		getSearchStore().mergedFilters = mergedFilters;
	}, [searchStore.filters]);
};
