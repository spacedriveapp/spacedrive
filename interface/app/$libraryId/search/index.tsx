import { useMemo } from 'react';
import { ObjectOrder, objectOrderingKeysSchema } from '@sd/client';
import { Icon } from '~/components';
import { useLocale, useRouteTitle } from '~/hooks';

import { SearchContextProvider, SearchOptions } from '.';
import Explorer from '../Explorer';
import { ExplorerContextProvider } from '../Explorer/Context';
import { createDefaultExplorerSettings } from '../Explorer/store';
import { DefaultTopBarOptions } from '../Explorer/TopBarOptions';
import { useExplorer, useExplorerSettings } from '../Explorer/useExplorer';
import { EmptyNotice } from '../Explorer/View/EmptyNotice';
import { TopBarPortal } from '../TopBar/Portal';
import SearchBar from './SearchBar';
import { useSearchFromSearchParams } from './useSearch';
import { useSearchExplorerQuery } from './useSearchExplorerQuery';

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

	const { t } = useLocale();

	const search = useSearchFromSearchParams({ defaultTarget: 'paths' });

	const items = useSearchExplorerQuery({
		search,
		explorerSettings,
		filters: search.allFilters,
		take: 100,
		objects: { order: explorerSettings.useSettingsSnapshot().order }
	});

	const explorer = useExplorer({
		...items,
		isFetchingNextPage: items.query.isFetchingNextPage,
		settings: explorerSettings
	});

	return (
		<ExplorerContextProvider explorer={explorer}>
			<SearchContextProvider search={search}>
				<TopBarPortal
					center={<SearchBar />}
					left={
						<div className="flex flex-row items-center gap-2">
							<span className="truncate text-sm font-medium">{t('search')}</span>
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
						message={t('no_items_found')}
					/>
				}
			/>
		</ExplorerContextProvider>
	);
}
