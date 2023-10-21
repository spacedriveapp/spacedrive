// import { useEffect, useState } from 'react';
// import { FilePathFilterArgs, ObjectKindEnum, useLibraryQuery } from '@sd/client';
// import { getSearchStore, SetFilter, useSearchStore } from '~/hooks';

// export interface SearchFilterOptions {
// 	locationId?: number;
// 	tags?: number[];
// 	objectKinds?: ObjectKindEnum[];
// }

// // Converts selected filters into a FilePathFilterArgs object for querying file paths
// const filtersToFilePathArgs = (filters: SetFilter[]): FilePathFilterArgs => {
// 	const filePathArgs: FilePathFilterArgs = {};

// 	// Iterate through selected filters and add them to the FilePathFilterArgs object
// 	filters.forEach((filter) => {
// 		switch (filter.categoryName) {
// 			case 'Location':
// 				filePathArgs.locationId = Number(filter.id);
// 				break;
// 			case 'Tagged':
// 				if (!filePathArgs.object) filePathArgs.object = {};
// 				if (!filePathArgs.object.tags) filePathArgs.object.tags = [];
// 				filePathArgs.object.tags.push(Number(filter.id));
// 				break;
// 			case 'Kind':
// 				if (!filePathArgs.object) filePathArgs.object = { kind: [] };
// 				filePathArgs.object.kind?.push(filter.id as unknown as ObjectKindEnum);
// 				break;
// 		}
// 	});

// 	return filePathArgs;
// };

// // Custom hook to manage search filters state and transform it to FilePathFilterArgs for further processing
// export const useSearchFilters = (options: SearchFilterOptions): FilePathFilterArgs => {
// 	const { locationId, tags, objectKinds } = options;
// 	const searchStore = useSearchStore();
// 	const [filePathArgs, setFilePathArgs] = useState<FilePathFilterArgs>({});

// 	useEffect(() => {
// 		const searchStore = getSearchStore();

// 		// If no filters are selected, initialize filters based on the provided options
// 		if (searchStore.selectedFilters.size === 0) {
// 			// handle location filter
// 			if (locationId) {
// 				const filter = searchStore.registerFilter(
// 					`${locationId}-${locationId}`,
// 					{ id: locationId, name: '', icon: 'Folder' },
// 					'Location'
// 				);
// 				searchStore.selectFilter(filter.key, true);
// 			}
// 			// handle tags filter
// 			tags?.forEach((tag) => {
// 				const tagFilter = searchStore.registerFilter(
// 					`${tag}-${tag}`,
// 					{ id: tag, name: `${tag}`, icon: `${tag}` },
// 					'Tag'
// 				);
// 				if (tagFilter) {
// 					searchStore.selectFilter(tagFilter.key, true);
// 				}
// 			});
// 			// handle object kinds filter
// 			objectKinds?.forEach((kind) => {
// 				const kindFilter = Array.from(searchStore.filters.values()).find(
// 					(filter) =>
// 						filter.categoryName === 'Kind' && filter.name === ObjectKindEnum[kind]
// 				);
// 				if (kindFilter) {
// 					searchStore.selectFilter(kindFilter.key, true);
// 				}
// 			});
// 		}

// 		// Convert selected filters to FilePathFilterArgs and update the state whenever selected filters change
// 		const selectedFiltersArray = Array.from(searchStore.selectedFilters.values());
// 		const updatedFilePathArgs = filtersToFilePathArgs(selectedFiltersArray);

// 		setFilePathArgs(updatedFilePathArgs);
// 		// eslint-disable-next-line react-hooks/exhaustive-deps
// 	}, [locationId, tags, objectKinds, searchStore.selectedFilters, searchStore.filters]);

// 	return filePathArgs;
// };
