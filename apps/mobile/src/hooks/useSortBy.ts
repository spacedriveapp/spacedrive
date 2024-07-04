import { FilePathOrder } from '@sd/client';
import { SortOptionsType, useSearchStore } from '~/stores/searchStore';

/**
 * This hook provides a sorting order object based on user preferences
 * for constructing the order query.
 */

export const useSortBy = (): FilePathOrder | null => {
	const searchStore = useSearchStore();
	const { by, direction } = searchStore.sort;

	// if no sort by field is selected, return null
	if (by === 'none') return null;

	// some sort by fields have common keys
	const common = { field: by, value: direction };

	const fields: Record<Exclude<SortOptionsType['by'], 'none'>, any> = {
		name: common,
		sizeInBytes: common,
		dateIndexed: common,
		dateCreated: common,
		dateModified: common,
		dateAccessed: { field: 'object', value: { field: 'dateAccessed', value: direction } },
		dateTaken: {
			field: 'object',
			value: { field: 'mediaData', value: { field: 'epochTime', value: direction } }
		}
	};

	return fields[by];
};
