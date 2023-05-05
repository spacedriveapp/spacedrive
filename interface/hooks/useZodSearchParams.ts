import { useCallback, useMemo } from 'react';
import { NavigateOptions, useSearchParams } from 'react-router-dom';
import { getParams } from 'remix-params-helper';
import { z } from 'zod';

export function useZodSearchParams<Z extends z.ZodType<Record<string, any>>>(schema: Z) {
	// eslint-disable-next-line no-restricted-syntax
	const [searchParams, setSearchParams] = useSearchParams();

	const typedSearchParams = useMemo(
		() => getParams(searchParams, schema),
		[searchParams, schema]
	);

	if (!typedSearchParams.success) throw typedSearchParams.errors;

	return [
		typedSearchParams.data,
		useCallback(
			(
				data: z.input<Z> | ((data: z.input<Z>) => z.infer<Z>),
				navigateOpts?: NavigateOptions
			) => {
				if (typeof data === 'function') {
					setSearchParams((params) => {
						const typedPrevParams = getParams(params, schema);

						if (!typedPrevParams.success) throw typedPrevParams.errors;

						return data(typedPrevParams.data);
					}, navigateOpts);
				} else {
					setSearchParams(data as any, navigateOpts);
				}
			},
			[setSearchParams, schema]
		)
	] as const;
}
