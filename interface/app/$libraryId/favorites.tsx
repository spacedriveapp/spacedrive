import { useMemo } from 'react';
import { ObjectOrder, objectOrderingKeysSchema } from '@sd/client';
import { Icon } from '~/components';
import { useLocale, useRouteTitle } from '~/hooks';

import Explorer from './Explorer';
import { ExplorerContextProvider } from './Explorer/Context';
import { createDefaultExplorerSettings } from './Explorer/store';
import { DefaultTopBarOptions } from './Explorer/TopBarOptions';
import { useExplorer, useExplorerSettings } from './Explorer/useExplorer';
import { EmptyNotice } from './Explorer/View/EmptyNotice';
import { SearchContextProvider, SearchOptions, useSearchFromSearchParams } from './search';
import SearchBar from './search/SearchBar';
import { useSearchExplorerQuery } from './search/useSearchExplorerQuery';
import { TopBarPortal } from './TopBar/Portal';

export function Component() {
	useRouteTitle('Favorites');

	const { t } = useLocale();

	const explorerSettings = useExplorerSettings({
		settings: useMemo(() => {
			return createDefaultExplorerSettings<ObjectOrder>({ order: null });
		}, []),
		orderingKeys: objectOrderingKeysSchema
	});

	const search = useSearchFromSearchParams({ defaultTarget: 'objects' });

	const defaultFilter = { object: { favorite: true } };

	const items = useSearchExplorerQuery({
		search,
		explorerSettings,
		filters: [
			...search.allFilters,
			// TODO: Add filter to search options
			defaultFilter
		],
		take: 100,
		objects: { order: explorerSettings.useSettingsSnapshot().order }
	});

	const explorer = useExplorer({
		...items,
		isFetchingNextPage: items.query.isFetchingNextPage,
		isFetching: items.query.isFetching,
		settings: explorerSettings
	});

	return (
		<ExplorerContextProvider explorer={explorer}>
			<SearchContextProvider search={search}>
				<TopBarPortal
					center={<SearchBar defaultFilters={[defaultFilter]} defaultTarget="objects" />}
					left={
						<div className="flex flex-row items-center gap-2">
							<span className="truncate text-sm font-medium">{t('favorites')}</span>
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
						icon={<Icon name="Heart" size={128} />}
						message={t('no_favorite_items')}
					/>
				}
			/>
		</ExplorerContextProvider>
	);
}
