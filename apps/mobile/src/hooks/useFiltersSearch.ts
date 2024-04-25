import { SearchFilterArgs } from '@sd/client';
import { useEffect } from 'react';
import { getSearchStore, SearchFilters, useSearchStore } from '~/stores/searchStore';

/**
 * This hook merges the selected filters from Filters page in order
 * to make query calls for saved searches and setups filters for the search
 * the data structure has been designed to match the desktop app
 */

export function useFiltersSearch() {
	const searchStore = useSearchStore();

	// this is a helper function to get the data from the filter based on the type
	// some filters return an object containing the id, and some are just an array of strings
	function getDataFromFilter(data: any, keyValue: 'id' | 'none') {
		return data.map((item: any) => (keyValue === 'id' ? item.id : item));
	}

	// this is a helper function creating the filter object for the search query
	function createFilter(
		filter: SearchFilters,
		arg: 'filePath' | 'object',
		key: 'contains' | 'in',
		value: any
	) {
		return {
			[arg]: {
				[filter]: {
					[key]: value
				}
			}
		};
	}

	//for merging the applied filters
	const mergedFilters = () => {
		//each filter is associated with a path or object
		const filePath = ['locations', 'name', 'hidden', 'extension'];
		const object = ['tags', 'kind'];
		return Object.entries(searchStore.filters)
			.map(([key, value]) => {
				const searchFilterKey = key as SearchFilters;
				if (Array.isArray(value)) {
					if (value.length === 0 || value[0] === '') return;
				}
				if (filePath.includes(searchFilterKey)) {
					if (searchFilterKey === 'name' || searchFilterKey === 'extension') {
						return createFilter(
							searchFilterKey,
							'filePath',
							'contains',
							getDataFromFilter(value, 'none')
						);
					} else if (searchFilterKey === 'hidden') {
						return {
							filePath: {
								[searchFilterKey]: value
							}
						};
					} else {
						return createFilter(
							searchFilterKey,
							'filePath',
							'in',
							getDataFromFilter(value, 'id')
						);
					}
				} else if (object.includes(searchFilterKey)) {
					return createFilter(
						searchFilterKey,
						'object',
						'in',
						getDataFromFilter(value, 'id')
					);
				}
			})
			.filter((filter) => filter !== undefined) as SearchFilterArgs[];
	};

	useEffect(() => {
		getSearchStore().mergedFilters = mergedFilters();
	}, [searchStore.filters]);
};
