import { RenderSearchFilter } from '.';
import { favoriteFilter, hiddenFilter } from './registry/BooleanFilters';
import {
	filePathDateCreated,
	filePathDateIndexed,
	filePathDateModified,
	mediaDateTaken,
	objectDateAccessed
} from './registry/DateFilters';
import { kindFilter } from './registry/KindFilter';
import { locationFilter } from './registry/LocationFilter';
import { tagsFilter } from './registry/TagsFilter';
import { extensionFilter, nameFilter } from './registry/TextFilters';

export const filterRegistry: ReadonlyArray<RenderSearchFilter<any>> = [
	// Put filters here
	locationFilter,
	tagsFilter,
	kindFilter,
	nameFilter,
	extensionFilter,
	filePathDateCreated,
	filePathDateModified,
	objectDateAccessed,
	filePathDateIndexed,
	mediaDateTaken,
	favoriteFilter,
	hiddenFilter
] as const;

export type FilterType = (typeof filterRegistry)[number]['name'];
