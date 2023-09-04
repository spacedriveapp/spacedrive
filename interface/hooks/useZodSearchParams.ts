import { useCallback, useMemo } from 'react';
import { NavigateOptions, useSearchParams } from 'react-router-dom';
import { getParams } from 'remix-params-helper';
import type { z } from 'zod';

export function useZodSearchParams<Z extends z.AnyZodObject>(schema: Z) {
	// eslint-disable-next-line no-restricted-syntax
	const [searchParams, setSearchParams] = useSearchParams();
	const typedSearchParams = useMemo(
		() => getParams(searchParams, schema),
		[searchParams, schema]
	);

	if (!typedSearchParams.success) throw typedSearchParams.errors;

	return [
		typedSearchParams.data as z.infer<Z>,
		useCallback(
			(
				data: z.input<Z> | ((data: z.input<Z>) => z.input<Z>),
				navigateOpts?: NavigateOptions
			) => {
				if (typeof data === 'function') {
					setSearchParams((params) => {
						const typedPrevParams = getParams(params, schema);

						if (!typedPrevParams.success) throw typedPrevParams.errors;

						return schema.parse(data(typedPrevParams.data));
					}, navigateOpts);
				} else {
					setSearchParams(data as any, navigateOpts);
				}
			},
			[setSearchParams, schema]
		)
	] as const;
}
