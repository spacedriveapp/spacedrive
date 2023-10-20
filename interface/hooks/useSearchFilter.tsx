import { useEffect, useState } from 'react';
import { FilePathFilterArgs, ObjectKindEnum, useLibraryQuery } from '@sd/client';
import { getSearchStore, SetFilter, useSearchStore } from '~/hooks';

export interface SearchFilterOptions {
	locationId?: number;
	tags?: number[];
	objectKinds?: ObjectKindEnum[];
}

const filtersToFilePathArgs = (filters: SetFilter[]): FilePathFilterArgs => {
	const filePathArgs: FilePathFilterArgs = {};

	filters.forEach((filter) => {
		switch (filter.categoryName) {
			case 'Location':
				filePathArgs.locationId = Number(filter.id);
				break;
			case 'Tagged':
				if (!filePathArgs.object) filePathArgs.object = {};
				if (!filePathArgs.object.tags) filePathArgs.object.tags = [];
				filePathArgs.object.tags.push(Number(filter.id));
				break;
			case 'Kind':
				if (!filePathArgs.object) filePathArgs.object = { kind: [] };
				filePathArgs.object.kind?.push(filter.id as unknown as ObjectKindEnum);
				break;
		}
	});

	return filePathArgs;
};

export const useSearchFilters = (options: SearchFilterOptions): FilePathFilterArgs => {
	const { locationId, tags, objectKinds } = options;
	const searchStore = useSearchStore();
	const [filePathArgs, setFilePathArgs] = useState<FilePathFilterArgs>({});

	useEffect(() => {
		const searchStore = getSearchStore();

		if (searchStore.selectedFilters.size === 0) {
			// Initialize with options if no filters are selected yet
			if (locationId) {
				const filter = searchStore.registerFilter(
					`${locationId}-${locationId}`,
					{ id: locationId, name: '', icon: 'Folder' },
					'Location'
				);

				searchStore.selectFilter(filter.key, true);
			}

			tags?.forEach((tag) => {
				const tagFilter = Array.from(searchStore.filters.values()).find(
					(filter) => filter.categoryName === 'Tagged' && Number(filter.id) === tag
				);
				if (tagFilter) {
					searchStore.selectFilter(tagFilter.key, true);
				}
			});

			objectKinds?.forEach((kind) => {
				const kindFilter = Array.from(searchStore.filters.values()).find(
					(filter) =>
						filter.categoryName === 'Kind' && filter.name === ObjectKindEnum[kind]
				);
				if (kindFilter) {
					searchStore.selectFilter(kindFilter.key, true);
				}
			});
		}

		const selectedFiltersArray = Array.from(searchStore.selectedFilters.values());
		const updatedFilePathArgs = filtersToFilePathArgs(selectedFiltersArray);
		setFilePathArgs(updatedFilePathArgs);
	}, [locationId, location, tags, objectKinds, searchStore.selectedFilters, searchStore.filters]);

	return filePathArgs;
};
