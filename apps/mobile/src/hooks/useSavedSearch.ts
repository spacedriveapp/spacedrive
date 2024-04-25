import { SavedSearch, useCache, useLibraryQuery, useNodes } from '@sd/client';
import { kinds } from '~/components/search/filters/Kind';
import { SearchFilters } from '~/stores/searchStore';

/**
 * This hook takes in the JSON of a Saved Search
 * and returns the data of its filters for rendering in the UI
 */

export function useSavedSearch(search: SavedSearch) {
	const parseFilters = JSON.parse(search.filters as string);

	// returns an array of keys of the filters being used in the Saved Search
	//i.e locations, tags, kind, etc...
	const filterKeys = parseFilters.reduce((acc: SearchFilters[], curr: any) => {
		const objectOrFilePath = Object.keys(curr)[0] as string;
		const key = Object.keys(curr[objectOrFilePath as SearchFilters])[0] as SearchFilters;
		if (!acc.includes(key)) {
			acc.push(key as SearchFilters);
		}
		return acc;
	}, []);

	function extractDataFromFilter(key: SearchFilters, filterTag: 'contains' | 'in', type: 'filePath' | 'object') {
		// Iterate through each item in the data array
		for (const item of parseFilters) {
			// Check if 'filePath' | 'object' exists and contains a the key
			if (item[type] && key in item[type]) {
				// Return the data of the filters
				return item.filePath[key][filterTag];
			}
		}
		return null;
	}

	const locationsQuery = useLibraryQuery(['locations.list'], {
		keepPreviousData: true,
		enabled: filterKeys.includes('locations')
	});
	const tagsQuery = useLibraryQuery(['tags.list'], {
		keepPreviousData: true,
		enabled: filterKeys.includes('tags')
	});

	useNodes(locationsQuery.data?.nodes);
	useNodes(tagsQuery.data?.nodes);

	const locations = useCache(locationsQuery.data?.items);
	const tags = useCache(tagsQuery.data?.items);

	// Filters like locations, tags, and kind require data to be rendered as a Filter
	// We prepare the data in the same format as the "filters" object in the "SearchStore"
	// it is then 'matched' with the data from the "Saved Search"

	const prepFilters = () => {
		const data = {} as Record<SearchFilters, any>;
		filterKeys.forEach((key: SearchFilters) => {
			switch (key) {
				case 'locations':
					data.locations = locations.map((location) => {
						return {
							id: location.id,
							name: location.name
						};
					});
					break;
				case 'tags':
					data.tags = tags.map((tag) => {
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
					data.name = extractDataFromFilter(key, 'contains', 'filePath');
					break;
			}
		});
		return data;
	};

	const filters = parseFilters.reduce((acc: Record<SearchFilters, {}>, curr: any) => {
		// this function extracts the data from the result of the "filters" object in the Saved Search
		// and matches it with the values of the filters
		const extractData = (key: SearchFilters) => {
			const objectOrFilePath = Object.keys(curr)[0] as string;
			const values = curr[objectOrFilePath as SearchFilters][key];
			const type = Object.keys(values)[0];

			switch (type) {
				case 'contains':
					return prepFilters()[key].filter((item: any) => item.name === values[type]);
				case 'in':
					return prepFilters()[key].filter((item: any) => values[type].includes(item.id));
				default:
					return values;
			}
		};

		const objectOrFilePath = Object.keys(curr)[0];

		// the data being setup for the filters so it can be rendered
		const filterKeys = Object.keys(curr[objectOrFilePath as any])[0] as SearchFilters;
		if (!acc[filterKeys]) {
			acc[filterKeys] = extractData(filterKeys);
			//don't include false values i.e if the "Hidden" filter is false
			if (acc[filterKeys] === false) {
				delete acc[filterKeys];
			}
		}
		return acc;
	}, {});

	return filters;
};
