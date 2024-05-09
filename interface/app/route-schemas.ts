import { z } from '@sd/ui/src/forms';

import i18n from './I18n';

export const SortOrderSchema = z.union([
	z.literal('Asc').describe(i18n.t('ascending')),
	z.literal('Desc').describe(i18n.t('descending'))
]);
export type SortOrder = z.infer<typeof SortOrderSchema>;

export const NodeIdParamsSchema = z.object({ id: z.string() });
export type NodeIdParams = z.infer<typeof NodeIdParamsSchema>;

export const LibraryIdParamsSchema = z.object({ libraryId: z.string().optional() });
export type LibraryIdParams = z.infer<typeof LibraryIdParamsSchema>;

export const LocationIdParamsSchema = z.object({ id: z.coerce.number() });
export type LocationIdParams = z.infer<typeof LocationIdParamsSchema>;

export const TagsSettingsParamsSchema = LocationIdParamsSchema.extend({
	id: LocationIdParamsSchema.shape.id.optional()
});
export type TagsSettingsParams = z.infer<typeof TagsSettingsParamsSchema>;

export const PathParamsSchema = z.object({ path: z.string().optional() });
export type PathParams = z.infer<typeof PathParamsSchema>;

export const SearchIdParamsSchema = z.object({ id: z.coerce.number() });
export type SearchIdParams = z.infer<typeof SearchIdParamsSchema>;

export const SearchParamsSchema = PathParamsSchema.extend({
	// take: z.coerce.number().default(100),
	// order: z
	// 	.union([
	// 		z.object({ field: z.literal('name'), value: SortOrderSchema }),
	// 		z.object({ field: z.literal('dateCreated'), value: SortOrderSchema })
	// 		// z.object({ field: z.literal('sizeInBytes'), value: SortOrderSchema })
	// 	])
	// 	.optional(),
	search: z.string().optional()
});
export type SearchParams = z.infer<typeof SearchParamsSchema>;

export const ExplorerParamsSchema = PathParamsSchema.extend({
	take: z.coerce.number().default(100)
});
export type ExplorerParams = z.infer<typeof ExplorerParamsSchema>;
