import { MagnifyingGlass } from '@phosphor-icons/react';
import { getIcon, iconNames } from '@sd/assets/util';
import { useMemo } from 'react';
import {
	FilePathOrder,
	SearchFilterArgs,
	useLibraryMutation,
	useLibraryQuery,
	usePathsExplorerQuery
} from '@sd/client';
import { Button } from '@sd/ui';
import { SearchIdParamsSchema } from '~/app/route-schemas';
import { useRouteTitle, useZodRouteParams } from '~/hooks';

import Explorer from '../Explorer';
import { ExplorerContextProvider } from '../Explorer/Context';
import {
	createDefaultExplorerSettings,
	explorerStore,
	filePathOrderingKeysSchema
} from '../Explorer/store';
import { DefaultTopBarOptions } from '../Explorer/TopBarOptions';
import { useExplorer, useExplorerSettings } from '../Explorer/useExplorer';
import { EmptyNotice } from '../Explorer/View/EmptyNotice';
import { SearchContextProvider, SearchOptions, useSearch, useSearchContext } from '../search';
import SearchBar from '../search/SearchBar';
import { TopBarPortal } from '../TopBar/Portal';

export const Component = () => {
	const { id } = useZodRouteParams(SearchIdParamsSchema);

	const savedSearch = useLibraryQuery(['search.saved.get', id], {
		suspense: true
	});

	useRouteTitle(savedSearch.data?.name ?? '');

	const explorerSettings = useExplorerSettings({
		settings: useMemo(() => {
			return createDefaultExplorerSettings<FilePathOrder>({
				order: { field: 'name', value: 'Asc' }
			});
		}, []),
		orderingKeys: filePathOrderingKeysSchema
	});

	const rawFilters = savedSearch.data?.filters;

	const dynamicFilters = useMemo(() => {
		if (rawFilters) return JSON.parse(rawFilters) as SearchFilterArgs[];
	}, [rawFilters]);

	const search = useSearch({
		open: true,
		search: savedSearch.data?.search ?? undefined,
		dynamicFilters
	});

	const paths = usePathsExplorerQuery({
		arg: { filters: search.allFilters, take: 50 },
		order: explorerSettings.useSettingsSnapshot().order,
		onSuccess: () => explorerStore.resetNewThumbnails()
	});

	const explorer = useExplorer({
		...paths,
		isFetchingNextPage: paths.query.isFetchingNextPage,
		settings: explorerSettings
	});

	return (
		<ExplorerContextProvider explorer={explorer}>
			<SearchContextProvider search={search}>
				<TopBarPortal
					center={<SearchBar />}
					left={
						<div className="flex flex-row items-center gap-2">
							<MagnifyingGlass className="text-ink-dull" weight="bold" size={18} />
							<span className="truncate text-sm font-medium">
								{savedSearch.data?.name}
							</span>
						</div>
					}
					right={<DefaultTopBarOptions />}
				>
					<hr className="w-full border-t border-sidebar-divider bg-sidebar-divider" />
					<SearchOptions>
						{(search.dynamicFilters !== dynamicFilters ||
							search.search !== savedSearch.data?.search) && (
							<SaveButton searchId={id} />
						)}
					</SearchOptions>
				</TopBarPortal>
			</SearchContextProvider>

			<Explorer
				emptyNotice={
					<EmptyNotice
						icon={<img className="size-32" src={getIcon(iconNames.FolderNoSpace)} />}
						message={
							search.search
								? `No results found for "${search.search}"`
								: 'Search for files...'
						}
					/>
				}
			/>
		</ExplorerContextProvider>
	);
};

function SaveButton({ searchId }: { searchId: number }) {
	const updateSavedSearch = useLibraryMutation(['search.saved.update']);

	const search = useSearchContext();

	return (
		<Button
			className="flex shrink-0 flex-row"
			size="xs"
			variant="dotted"
			onClick={() => {
				updateSavedSearch.mutate([
					searchId,
					{
						filters: JSON.stringify(search.dynamicFilters),
						search: search.search
					}
				]);
			}}
		>
			Save
		</Button>
	);
}
