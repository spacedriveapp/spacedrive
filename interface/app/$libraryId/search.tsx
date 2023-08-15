import { MagnifyingGlass } from 'phosphor-react';
import { Suspense, memo, useDeferredValue, useMemo } from 'react';
import { getExplorerItemData, useLibraryQuery } from '@sd/client';
import { SearchParams, SearchParamsSchema } from '~/app/route-schemas';
import { useZodSearchParams } from '~/hooks';
import Explorer from './Explorer';
import { ExplorerContext } from './Explorer/Context';
import { DefaultTopBarOptions } from './Explorer/TopBarOptions';
import { EmptyNotice } from './Explorer/View';
import { getExplorerStore, useExplorerStore } from './Explorer/store';
import { useExplorer } from './Explorer/useExplorer';
import { TopBarPortal } from './TopBar/Portal';

const SearchExplorer = memo((props: { args: SearchParams }) => {
	const explorerStore = useExplorerStore();

	const { search, ...args } = props.args;

	const query = useLibraryQuery(['search.paths', { ...args, filter: { search } }], {
		suspense: true,
		enabled: !!search,
		onSuccess: () => getExplorerStore().resetNewThumbnails()
	});

	const items = useMemo(() => {
		const items = query.data?.items ?? null;

		if (explorerStore.layoutMode !== 'media') return items;

		return (
			items?.filter((item) => {
				const { kind } = getExplorerItemData(item);
				return kind === 'Video' || kind === 'Image';
			}) || null
		);
	}, [query.data, explorerStore.layoutMode]);

	const explorer = useExplorer({
		items
	});

	return (
		<>
			{search ? (
				<ExplorerContext.Provider value={explorer}>
					<TopBarPortal right={<DefaultTopBarOptions />} />
					<Explorer
						emptyNotice={<EmptyNotice message={`No results found for "${search}"`} />}
					/>
				</ExplorerContext.Provider>
			) : (
				<div className="flex flex-1 flex-col items-center justify-center">
					<MagnifyingGlass size={110} className="mb-5 text-ink-faint" opacity={0.3} />
					<p className="text-xs text-ink-faint">Search for files...</p>
				</div>
			)}
		</>
	);
});

export const Component = () => {
	const [searchParams] = useZodSearchParams(SearchParamsSchema);

	const search = useDeferredValue(searchParams);

	return (
		<Suspense>
			<SearchExplorer args={search} />
		</Suspense>
	);
};
