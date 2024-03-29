import { useMemo } from 'react';
import { ObjectKindEnum, ObjectOrder, SearchFilterArgs, useObjectsExplorerQuery } from '@sd/client';
import { Icon } from '~/components';
import { useRouteTitle } from '~/hooks';

import Explorer from './Explorer';
import { ExplorerContextProvider } from './Explorer/Context';
import { createDefaultExplorerSettings, objectOrderingKeysSchema } from './Explorer/store';
import { DefaultTopBarOptions } from './Explorer/TopBarOptions';
import { useExplorer, useExplorerSettings } from './Explorer/useExplorer';
import { EmptyNotice } from './Explorer/View/EmptyNotice';
import {
	SearchContextProvider,
	SearchOptions,
	useSearch,
	useSearchFromSearchParams
} from './search';
import SearchBar from './search/SearchBar';
import { TopBarPortal } from './TopBar/Portal';

export function Component() {
	useRouteTitle('Recents');

	const explorerSettings = useExplorerSettings({
		settings: useMemo(() => {
			return createDefaultExplorerSettings<ObjectOrder>({ order: null });
		}, []),
		orderingKeys: objectOrderingKeysSchema
	});

	const explorerSettingsSnapshot = explorerSettings.useSettingsSnapshot();

	const search = useSearchFromSearchParams();

	const objects = useObjectsExplorerQuery({
		arg: {
			take: 100,
			filters: [
				...search.allFilters,
				// TODO: Add fil ter to search options
				{ object: { dateAccessed: { from: new Date(0).toISOString() } } },
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
							<span className="truncate text-sm font-medium">Recents</span>
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
						message="Recents are created when you open a file."
					/>
				}
			/>
		</ExplorerContextProvider>
	);
}
