import { useMemo } from 'react';
import { useParams } from 'react-router';
import type { z } from 'zod';

export function useZodRouteParams<Z extends z.AnyZodObject>(schema: Z): z.infer<Z> {
	// eslint-disable-next-line no-restricted-syntax
	const params = useParams();
	return useMemo(() => schema.parse(params), [params, schema]);
}
