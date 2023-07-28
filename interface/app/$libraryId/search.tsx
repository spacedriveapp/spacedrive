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
import { TopBarPortal } from './TopBar/Portal';

const SearchExplorer = memo((props: { args: SearchParams }) => {
	const explorerStore = useExplorerStore();

	const { search, ...args } = props.args;

	const query = useLibraryQuery(['search.paths', { ...args, filter: { search } }], {
		suspense: true,
		enabled: !!search,
		onSuccess: () => getExplorerStore().resetNewThumbnails()
	});

	const items =
		useMemo(() => {
			const items = query.data?.items;

			if (explorerStore.layoutMode !== 'media') return items;

			return items?.filter((item) => {
				const { kind } = getExplorerItemData(item);
				return kind === 'Video' || kind === 'Image';
			});
		}, [query.data, explorerStore.layoutMode]) ?? [];

	return (
		<ExplorerContext.Provider value={{}}>
			<TopBarPortal right={<DefaultTopBarOptions />} />
			<Explorer
				items={items}
				emptyNotice={
					<EmptyNotice
						icon={
							search || (
								<MagnifyingGlass
									size={110}
									className="mb-5 text-ink-faint"
									opacity={0.3}
								/>
							)
						}
						message={
							search ? `No results found for "${search}"` : 'Search for files...'
						}
					/>
				}
			/>
		</ExplorerContext.Provider>
	);
});

export const Component = () => {
	const [searchParams] = useZodSearchParams(SearchParamsSchema);

	const search = useDeferredValue(searchParams);

	return (
		<Suspense fallback="LOADING FIRST RENDER">
			<SearchExplorer args={search} />
		</Suspense>
	);
};
