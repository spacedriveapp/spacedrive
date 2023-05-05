import { useMemo } from 'react';
import { useParams } from 'react-router';
import { z } from 'zod';

export function useZodRouteParams<Z extends z.ZodType<Record<string, any>>>(schema: Z) {
	// eslint-disable-next-line no-restricted-syntax
	const params = useParams();

	return useMemo(() => schema.parse(params), [params, schema]);
}
