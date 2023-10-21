import { MagnifyingGlass } from '@phosphor-icons/react';
import { useEffect, useMemo } from 'react';
import { FilePathFilterArgs, useLibraryContext } from '@sd/client';
import { SearchParams, SearchParamsSchema } from '~/app/route-schemas';
import { useZodSearchParams } from '~/hooks';

import Explorer from './Explorer';
import { ExplorerContextProvider } from './Explorer/Context';
import { usePathsInfiniteQuery } from './Explorer/queries';
import { createDefaultExplorerSettings, filePathOrderingKeysSchema } from './Explorer/store';
import { DefaultTopBarOptions } from './Explorer/TopBarOptions';
import { useExplorer, useExplorerSettings } from './Explorer/useExplorer';
import { EmptyNotice } from './Explorer/View';
import {
	getSearchStore,
	useSavedSearches,
	useSearchFilters
} from './Explorer/View/SearchOptions/store';
import { TopBarPortal } from './TopBar/Portal';

const useItems = (searchParams: SearchParams) => {
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
		if (searchParams.savedSearchKey) {
			getSearchStore().isSearching = true;
			savedSearches.loadSearch(searchParams.savedSearchKey);
		}
	}, []);

	const filter: FilePathFilterArgs = {
		search: searchParams.search,
		...searchFilters
	};

	const take = 50; // Specify the number of items to fetch per query

	const query = usePathsInfiniteQuery({
		arg: { filter, take },
		library,
		settings: explorerSettings
	});

	const items = useMemo(() => query.data?.pages.flatMap((d) => d.items) ?? [], [query.data]);

	return { items, query };
};

const SearchExplorer = ({ args }: { args: SearchParams }) => {
	const { items, query } = useItems(args);

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
			<TopBarPortal right={<DefaultTopBarOptions />} />
			<Explorer
				emptyNotice={
					<EmptyNotice
						icon={
							!args.search ? (
								<MagnifyingGlass
									size={110}
									className="mb-5 text-ink-faint"
									opacity={0.3}
								/>
							) : null
						}
						message={
							args.search
								? `No results found for "${args.search}"`
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

	return <SearchExplorer args={searchParams} />;
};
