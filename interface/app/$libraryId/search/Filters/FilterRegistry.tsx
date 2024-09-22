import { RenderSearchFilter } from '.';
import { filePathDateCreated } from './registry/DateFilters';
import { kindFilter } from './registry/KindFilter';
import { locationFilter } from './registry/LocationFilter';
import { tagsFilter } from './registry/TagsFilter';
import { extensionFilter, nameFilter } from './registry/TextFilters';

export const filterRegistry: ReadonlyArray<RenderSearchFilter<any>> = [
	// Put filters here
	locationFilter,
	filePathDateCreated,
	tagsFilter,
	kindFilter,
	nameFilter,
	extensionFilter
] as const;

export type FilterType = (typeof filterRegistry)[number]['name'];
