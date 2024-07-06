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
	useRouteTitle('Recents');

	const explorerSettings = useExplorerSettings({
		settings: useMemo(() => {
			return createDefaultExplorerSettings<ObjectOrder>({ order: null });
		}, []),
		orderingKeys: objectOrderingKeysSchema
	});

	const search = useSearchFromSearchParams({ defaultTarget: 'paths' });

	const { t } = useLocale();

	const defaultFilters = { object: { dateAccessed: { from: new Date(0).toISOString() } } };

	const items = useSearchExplorerQuery({
		search,
		explorerSettings,
		filters: [
			...search.allFilters,
			// TODO: Add fil ter to search options
			defaultFilters
		],
		take: 100,
		objects: { order: explorerSettings.useSettingsSnapshot().order }
	});

	const explorer = useExplorer({
		...items,
		isFetching: items.query.isFetching,
		isFetchingNextPage: items.query.isFetchingNextPage,
		settings: explorerSettings
	});
	return (
		<ExplorerContextProvider explorer={explorer}>
			<SearchContextProvider search={search}>
				<TopBarPortal
					center={<SearchBar defaultFilters={[defaultFilters]} defaultTarget="paths" />}
					left={
						<div className="flex flex-row items-center gap-2">
							<span className="truncate text-sm font-medium">{t('recents')}</span>
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
						icon={<Icon name="Collection" size={128} />}
						message={t('recents_notice_message')}
					/>
				}
			/>
		</ExplorerContextProvider>
	);
}
