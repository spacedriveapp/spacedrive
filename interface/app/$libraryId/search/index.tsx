import { useEffect, useMemo } from 'react';
import { useSearchParams as useRawSearchParams } from 'react-router-dom';
import { ObjectKindEnum, ObjectOrder, useObjectsExplorerQuery } from '@sd/client';
import { Icon } from '~/components';
import { useRouteTitle } from '~/hooks';

import { SearchContextProvider, SearchOptions, useSearch } from '.';
import Explorer from '../Explorer';
import { ExplorerContextProvider } from '../Explorer/Context';
import { createDefaultExplorerSettings, objectOrderingKeysSchema } from '../Explorer/store';
import { DefaultTopBarOptions } from '../Explorer/TopBarOptions';
import { useExplorer, UseExplorerSettings, useExplorerSettings } from '../Explorer/useExplorer';
import { EmptyNotice } from '../Explorer/View/EmptyNotice';
import { TopBarPortal } from '../TopBar/Portal';
import SearchBar from './SearchBar';

export * from './context';
export * from './SearchOptions';
export * from './useSearch';

export function Component() {
	useRouteTitle('Search');

	const explorerSettings = useExplorerSettings({
		settings: useMemo(() => {
			return createDefaultExplorerSettings<ObjectOrder>({ order: null });
		}, []),
		orderingKeys: objectOrderingKeysSchema
	});
	const explorerSettingsSnapshot = explorerSettings.useSettingsSnapshot();

	const search = useSearchWithFilters();

	const objects = useObjectsExplorerQuery({
		arg: {
			take: 100,
			filters: [
				...search.allFilters,
				...(explorerSettingsSnapshot.layoutMode === 'media'
					? [{ object: { kind: { in: [ObjectKindEnum.Image, ObjectKindEnum.Video] } } }]
					: [])
			]
		},
		order: explorerSettings.useSettingsSnapshot().order
	});

	const explorer = useExplorer({
		...objects,
		isFetchingNextPage: objects.query.isFetchingNextPage,
		settings: explorerSettings
	});

	return (
		<ExplorerContextProvider explorer={explorer}>
			<SearchContextProvider search={search}>
				<TopBarPortal
					center={<SearchBar />}
					left={
						<div className="flex flex-row items-center gap-2">
							<span className="truncate text-sm font-medium">Search</span>
						</div>
					}
					right={<DefaultTopBarOptions />}
				>
					{search.open && (
						<>
							<hr className="w-full border-t border-sidebar-divider bg-sidebar-divider" />
							<SearchOptions />
						</>
					)}
				</TopBarPortal>
			</SearchContextProvider>

			<Explorer
				emptyNotice={
					<EmptyNotice
						icon={<Icon name="Search" size={128} />}
						message="No items found"
					/>
				}
			/>
		</ExplorerContextProvider>
	);
}

function useSearchWithFilters() {
	const [searchParams, setSearchParams] = useRawSearchParams();

	const filtersParam = searchParams.get('filters');
	const filters = useMemo(() => JSON.parse(filtersParam ?? '[]'), [filtersParam]);

	const searchQueryParam = searchParams.get('search');

	const search = useSearch({
		open: !!searchQueryParam || filters.length > 0 || undefined,
		search: searchParams.get('search') ?? undefined,
		filters
	});

	useEffect(() => {
		setSearchParams(
			(p) => {
				if (search.filters.length > 0) p.set('filters', JSON.stringify(search.filters));
				else p.delete('filters');

				return p;
			},
			{ replace: true }
		);
	}, [search.filters, setSearchParams]);

	const searchQuery = search.search;

	useEffect(() => {
		setSearchParams(
			(p) => {
				if (searchQuery !== '') p.set('search', searchQuery);
				else p.delete('search');

				return p;
			},
			{ replace: true }
		);
		// Do not add setSearchParams to the dependencies array, it will cause CMDK to not navigate to search page (multiple times)
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [searchQuery]);

	return search;
}
