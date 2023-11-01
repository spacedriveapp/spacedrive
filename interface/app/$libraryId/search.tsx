import { MagnifyingGlass } from '@phosphor-icons/react';
import { getIcon, iconNames } from '@sd/assets/util';
import { Suspense, useDeferredValue, useEffect, useMemo } from 'react';
import { FilePathFilterArgs, useLibraryContext } from '@sd/client';
import { SearchIdParamsSchema, SearchParams, SearchParamsSchema } from '~/app/route-schemas';
import { useZodRouteParams, useZodSearchParams } from '~/hooks';

import Explorer from './Explorer';
import { ExplorerContextProvider } from './Explorer/Context';
import { usePathsInfiniteQuery } from './Explorer/queries';
import { createDefaultExplorerSettings, filePathOrderingKeysSchema } from './Explorer/store';
import { DefaultTopBarOptions } from './Explorer/TopBarOptions';
import { useExplorer, useExplorerSettings } from './Explorer/useExplorer';
import { EmptyNotice } from './Explorer/View';
import { useSavedSearches } from './Explorer/View/SearchOptions/SavedSearches';
import { getSearchStore, useSearchFilters } from './Explorer/View/SearchOptions/store';
import { TopBarPortal } from './TopBar/Portal';

const useItems = (searchParams: SearchParams, id: number) => {
	const { library } = useLibraryContext();
	const explorerSettings = useExplorerSettings({
		settings: createDefaultExplorerSettings({
			order: {
				field: 'name',
				value: 'Asc'
			}
		}),
		orderingKeys: filePathOrderingKeysSchema
	});

	const searchFilters = useSearchFilters('paths', []);

	const savedSearches = useSavedSearches();

	useEffect(() => {
		if (id) {
			getSearchStore().isSearching = true;
			savedSearches.loadSearch(id);
		}
	}, [id]);

	const take = 50; // Specify the number of items to fetch per query

	const query = usePathsInfiniteQuery({
		arg: { filter: searchFilters, take },
		library,
		// @ts-ignore todo: fix
		settings: explorerSettings,
		suspense: true
	});

	const items = useMemo(() => query.data?.pages.flatMap((d) => d.items) ?? [], [query.data]);

	return { items, query };
};

const SearchExplorer = ({ id, searchParams }: { id: number; searchParams: SearchParams }) => {
	const { items, query } = useItems(searchParams, id);

	const explorerSettings = useExplorerSettings({
		settings: createDefaultExplorerSettings({
			order: {
				field: 'name',
				value: 'Asc'
			}
		}),
		orderingKeys: filePathOrderingKeysSchema
	});

	const explorer = useExplorer({
		items,
		settings: explorerSettings
	});

	return (
		<ExplorerContextProvider explorer={explorer}>
			<TopBarPortal
				left={
					<div className="flex flex-row items-center gap-2">
						<MagnifyingGlass className="text-ink-dull" weight="bold" size={18} />
						<span className="truncate text-sm font-medium">Search</span>
					</div>
				}
				right={<DefaultTopBarOptions />}
			/>
			<Explorer
				showFilterBar
				emptyNotice={
					<EmptyNotice
						icon={<img className="h-32 w-32" src={getIcon(iconNames.FolderNoSpace)} />}
						message={
							searchParams.search
								? `No results found for "${searchParams.search}"`
								: 'Search for files...'
						}
					/>
				}
			/>
		</ExplorerContextProvider>
	);
};

export const Component = () => {
	const [searchParams] = useZodSearchParams(SearchParamsSchema);
	const { id } = useZodRouteParams(SearchIdParamsSchema);

	const search = useDeferredValue(searchParams);

	return (
		<Suspense>
			<SearchExplorer id={id} searchParams={search} />
		</Suspense>
	);
};
