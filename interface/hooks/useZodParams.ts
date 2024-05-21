import { useMemo } from 'react';
import { useParams } from 'react-router';
import { z } from 'zod';

export function useZodParams<Z extends z.AnyZodObject>(schema: Z): z.infer<Z> {
	// eslint-disable-next-line
	const params = useParams();

	return useMemo(() => schema.parse(params), [schema, params]);
}
