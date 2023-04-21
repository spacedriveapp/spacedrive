import { Suspense, memo, useDeferredValue, useMemo } from 'react';
import { useSearchParams } from 'react-router-dom';
import { z } from 'zod';
import { useLibraryQuery } from '@sd/client';

const schema = z.object({
	search: z.string().optional(),
	take: z.number().optional(),
	order: z.union([z.object({ name: z.boolean() }), z.object({ name: z.boolean() })]).optional()
});

export type SearchArgs = z.infer<typeof schema>;

const ExplorerStuff = memo((props: { args: SearchArgs }) => {
	const query = useLibraryQuery(['search', props.args], {
		suspense: true
	});

	return (
		<div className="page-scroll custom-scroll flex flex-col space-y-5">
			<code>
				<pre>{JSON.stringify(query.data, null, 4)}</pre>
			</code>
		</div>
	);
});

export const Component = () => {
	const [searchParams] = useSearchParams();

	const searchObj = useMemo(
		() => schema.parse(Object.fromEntries([...searchParams])),
		[searchParams]
	);

	const search = useDeferredValue(searchObj);

	return (
		<Suspense fallback="LOADING FIRST RENDER">
			<ExplorerStuff args={search} />
		</Suspense>
	);
};
