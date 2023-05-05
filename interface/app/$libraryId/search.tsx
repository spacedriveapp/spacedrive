import { MagnifyingGlass } from 'phosphor-react';
import { Suspense, memo, useDeferredValue, useEffect, useMemo } from 'react';
import { z } from 'zod';
import { useLibraryQuery } from '@sd/client';
import { useZodSearchParams } from '~/hooks';
import { getExplorerStore, useExplorerStore } from '~/hooks/useExplorerStore';
import { useExplorerTopBarOptions } from '~/hooks/useExplorerTopBarOptions';
import Explorer from './Explorer';
import { getExplorerItemData } from './Explorer/util';
import TopBarChildren from './TopBar/TopBarChildren';

const SEARCH_PARAMS = z.object({
	search: z.string().optional(),
	take: z.coerce.number().optional(),
	order: z.union([z.object({ name: z.boolean() }), z.object({ name: z.boolean() })]).optional()
});

export type SearchArgs = z.infer<typeof SEARCH_PARAMS>;

const ExplorerStuff = memo((props: { args: SearchArgs }) => {
	const explorerStore = useExplorerStore();
	const { explorerViewOptions, explorerControlOptions } = useExplorerTopBarOptions();

	const query = useLibraryQuery(['search', props.args], {
		suspense: true,
		enabled: !!props.args.search
	});

	const items = useMemo(() => {
		if (explorerStore.layoutMode !== 'media') return query.data;

		return query.data?.filter((item) => {
			const { kind } = getExplorerItemData(item);
			return kind === 'Video' || kind === 'Image';
		});
	}, [query.data, explorerStore.layoutMode]);

	useEffect(() => {
		getExplorerStore().selectedRowIndex = null;
	}, [props.args.search]);

	return (
		<>
			{items && items.length > 0 ? (
				<>
					<TopBarChildren toolOptions={[explorerViewOptions, explorerControlOptions]} />
					<Explorer items={items} />
				</>
			) : (
				<div className="flex flex-1 flex-col items-center justify-center">
					{!props.args.search && (
						<MagnifyingGlass size={110} className="mb-5 text-ink-faint" opacity={0.3} />
					)}
					<p className="text-xs text-ink-faint">
						{props.args.search
							? `No results found for "${props.args.search}"`
							: 'Search for files...'}
					</p>
				</div>
			)}
		</>
	);
});

export const Component = () => {
	const [searchParams] = useZodSearchParams(SEARCH_PARAMS);

	const search = useDeferredValue(searchParams);

	return (
		<Suspense fallback="LOADING FIRST RENDER">
			<ExplorerStuff args={search} />
		</Suspense>
	);
};
