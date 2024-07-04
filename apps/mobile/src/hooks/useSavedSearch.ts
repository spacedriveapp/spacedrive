import { useCallback, useMemo } from 'react';
import { SavedSearch, SearchFilterArgs, useLibraryQuery } from '@sd/client';
import { kinds } from '~/components/search/filters/Kind';
import { Filters, SearchFilters } from '~/stores/searchStore';

/**
 * This hook takes in the JSON of a Saved Search
 * and returns the data of its filters for rendering in the UI
 */

export function useSavedSearch(search: SavedSearch) {
	const parseFilters = JSON.parse(search.filters as string);

	// returns an array of keys of the filters being used in the Saved Search
	//i.e locations, tags, kind, etc...
	const filterKeys: SearchFilters[] = parseFilters.reduce(
		(acc: SearchFilters[], curr: keyof SearchFilterArgs) => {
			const objectOrFilePath = Object.keys(curr)[0] as 'filePath' | 'object';
			const key = Object.keys(curr[objectOrFilePath])[0] as SearchFilters;
			if (!acc.includes(key)) {
				acc.push(key as SearchFilters);
			}
			return acc;
		},
		[]
	);

	// this util function extracts the data of a filter from the Saved Search
	const extractDataFromSavedSearch = (
		key: SearchFilters,
		filterTag: 'contains' | 'in',
		type: 'filePath' | 'object'
	) => {
		// Iterate through each item in the data array
		for (const item of parseFilters) {
			// Check if 'filePath' | 'object' exists and contains a the key
			if (item[type] && key in item[type]) {
				// Return the data of the filters
				return item.filePath[key][filterTag];
			}
		}
		return null;
	};

	const locations = useLibraryQuery(['locations.list'], {
		keepPreviousData: true,
		enabled: filterKeys.includes('locations')
	});
	const tags = useLibraryQuery(['tags.list'], {
		keepPreviousData: true,
		enabled: filterKeys.includes('tags')
	});

	// Filters like locations, tags, and kind require data to be rendered as a Filter
	// We prepare the data in the same format as the "filters" object in the "SearchStore"
	// it is then 'matched' with the data from the "Saved Search"

	const prepFilters = useCallback(() => {
		const data = {} as Record<SearchFilters, any>;
		filterKeys.forEach((key: SearchFilters) => {
			switch (key) {
				case 'locations':
					data.locations = locations.data?.map((location) => {
						return {
							id: location.id,
							name: location.name
						};
					});
					break;
				case 'tags':
					data.tags = tags.data?.map((tag) => {
						return {
							id: tag.id,
							color: tag.color
						};
					});
					break;
				case 'kind':
					data.kind = kinds.map((kind) => {
						return {
							name: kind.name,
							id: kind.value,
							icon: kind.icon
						};
					});
					break;
				case 'name':
					data.name = extractDataFromSavedSearch(key, 'contains', 'filePath');
					break;
				case 'extension':
					data.extension = extractDataFromSavedSearch(key, 'contains', 'filePath');
					break;
			}
		});
		return data;
	}, [locations, tags]);

	const filters: Partial<Filters> = useMemo(() => {
		return parseFilters.reduce(
			(acc: Record<SearchFilters, {}>, curr: keyof SearchFilterArgs) => {
				const objectOrFilePath = Object.keys(curr)[0] as 'filePath' | 'object';
				const key = Object.keys(curr[objectOrFilePath])[0] as SearchFilters; //locations, tags, kind, etc...

				// this function extracts the data from the result of the "filters" object in the Saved Search
				// and matches it with the values of the filters
				const extractData = (key: SearchFilters) => {
					const values: {
						contains?: string;
						in?: number[];
					} = curr[objectOrFilePath][key];
					const type = Object.keys(values)[0];

					switch (type) {
						case 'contains':
							// some filters have a name property and some are just strings
							return prepFilters()[key].filter((item: any) => {
								return item.name ? item.name === values[type] : item;
							});
						case 'in':
							return prepFilters()[key].filter((item: any) =>
								values[type]?.includes(item.id)
							);
						default:
							return values;
					}
				};

				// the data being setup for the filters so it can be rendered
				if (!acc[key]) {
					acc[key] = extractData(key);
					//don't include false values i.e if the "Hidden" filter is false
					if (acc[key] === false) {
						delete acc[key];
					}
				}
				return acc;
			},
			{}
		);
	}, [parseFilters]);

	return filters;
}
