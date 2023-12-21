import { useMemo } from 'react';
import { ObjectFilterArgs, ObjectKindEnum, ObjectOrder, SearchFilterArgs } from '@sd/client';
import { Icon } from '~/components';
import { useRouteTitle } from '~/hooks';

import Explorer from './Explorer';
import { ExplorerContextProvider } from './Explorer/Context';
import { useObjectsExplorerQuery } from './Explorer/queries/useObjectsExplorerQuery';
import { createDefaultExplorerSettings, objectOrderingKeysSchema } from './Explorer/store';
import { DefaultTopBarOptions } from './Explorer/TopBarOptions';
import { useExplorer, useExplorerSettings } from './Explorer/useExplorer';
import { EmptyNotice } from './Explorer/View/EmptyNotice';
import SearchOptions, { SearchContextProvider, useSearch } from './Search';
import SearchBar from './Search/SearchBar';
import { TopBarPortal } from './TopBar/Portal';

export function Component() {
	useRouteTitle('Labels');

	const explorerSettings = useExplorerSettings({
		settings: useMemo(() => {
			return createDefaultExplorerSettings<ObjectOrder>({ order: null });
		}, []),
		orderingKeys: objectOrderingKeysSchema
	});

	const explorerSettingsSnapshot = explorerSettings.useSettingsSnapshot();

	const fixedFilters = useMemo<SearchFilterArgs[]>(
		() => [
			...(explorerSettingsSnapshot.layoutMode === 'media'
				? [{ object: { kind: { in: [ObjectKindEnum.Image, ObjectKindEnum.Video] } } }]
				: [])
		],
		[explorerSettingsSnapshot.layoutMode]
	);

	const search = useSearch({
		fixedFilters
	});

	const objects = useObjectsExplorerQuery({
		arg: {
			take: 100,
			filters: [...search.allFilters, { object: { tags: { in: [3] } } }]
		},
		explorerSettings
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
							<span className="truncate text-sm font-medium">Labels</span>
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
						icon={<Icon name="CollectionSparkle" size={128} />}
						message="No labels"
					/>
				}
			/>
		</ExplorerContextProvider>
	);
}
