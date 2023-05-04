import { useMemo } from 'react';
import { useParams } from 'react-router';
import { z } from 'zod';

export function useZodRouteParams<Z extends z.ZodType>(schema: Z): z.infer<Z> {
	const params = useParams();

	return useMemo(() => schema.parse(params), [params, schema]);
}
