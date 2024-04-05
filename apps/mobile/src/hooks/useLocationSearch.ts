import { useEffect } from 'react';
import { getSearchStore, SearchFilters, useSearchStore } from '~/stores/searchStore';

export const useLocationSearch = () => {
	const searchStore = useSearchStore();

	// this is a helper function to get the data from the filter based on the type
	// some filters return an object containing the id, and some are just an array of strings
	function getDataFromFilter(data: any, keyValue: 'id' | 'none') {
		return data.map((item: any) => (keyValue === 'id' ? item.id : item));
	}

	// this is a helper function createing the filter object for the search query
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

	//for merging the applied filters with the search create query
	const mergedFilters = () => {
		//each filter is associated with a path or object
		const filePath = ['locations', 'name', 'hidden', 'extension'];
		const object = ['tags', 'kind'];
		return Object.entries(searchStore.filters)
			.map(([key, value]) => {
				if (Array.isArray(value)) {
					if (value.length === 0 || value[0] === '') return;
				}
				if (filePath.includes(key as SearchFilters)) {
					if (key === 'name' || key === 'extension') {
						return createFilter(
							key as SearchFilters,
							'filePath',
							'contains',
							getDataFromFilter(value, 'none')
						);
					} else if (key === 'hidden') {
						return {
							filePath: {
								[key]: value
							}
						};
					} else {
						return createFilter(
							key as SearchFilters,
							'filePath',
							'in',
							getDataFromFilter(value, 'id')
						);
					}
				} else if (object.includes(key as SearchFilters)) {
					return createFilter(
						key as SearchFilters,
						'object',
						'in',
						getDataFromFilter(value, 'id')
					);
				}
			})
			.filter((filter) => filter !== undefined);
	};

	console.log(getSearchStore().mergedFilters, 'merged filters');

	useEffect(() => {
		getSearchStore().mergedFilters = mergedFilters();
	}, [searchStore.filters]);
};
