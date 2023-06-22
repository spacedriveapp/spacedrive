import { useMemo } from 'react';
import { useParams } from 'react-router';
import { z } from 'zod';

const RouteParamsSchema = z.object({
	id: z.coerce.number().default(0),
	libraryId: z.string().optional()
});

export function useZodRouteParams<Z extends z.AnyZodObject = typeof RouteParamsSchema>(
	schema?: Z
): z.infer<Z> {
	// eslint-disable-next-line no-restricted-syntax
	const params = useParams();
	return useMemo(() => (schema ?? RouteParamsSchema).parse(params), [params, schema]);
}
