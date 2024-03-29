import { useMemo } from 'react';
import { ObjectOrder, useLibraryQuery } from '@sd/client';
import { Icon } from '~/components';
import { useLocale, useRouteTitle } from '~/hooks';

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
	useRouteTitle('Labels');

	const labels = useLibraryQuery(['labels.listWithThumbnails', '']);

	const explorerSettings = useExplorerSettings({
		settings: useMemo(() => {
			return createDefaultExplorerSettings<ObjectOrder>({ order: null });
		}, []),
		orderingKeys: objectOrderingKeysSchema
	});

	// const explorerSettingsSnapshot = explorerSettings.useSettingsSnapshot();

	// const filters = useMemo<SearchFilterArgs[]>(
	// 	() => [
	// 		...(explorerSettingsSnapshot.layoutMode === 'media'
	// 			? [{ object: { kind: { in: [ObjectKindEnum.Image, ObjectKindEnum.Video] } } }]
	// 			: [])
	// 	],
	// 	[explorerSettingsSnapshot.layoutMode]
	// );

	const search = useSearchFromSearchParams();

	// const objects = useObjectsExplorerQuery({
	// 	arg: {
	// 		take: 100,
	// 		filters: [...search.allFilters, { object: { tags: { in: [3] } } }]
	// 	},
	// 	explorerSettings
	// });

	const explorer = useExplorer({
		items: labels.data || null,
		settings: explorerSettings,
		showPathBar: false,
		layouts: { media: false, list: false }
	});

	const { t } = useLocale();

	return (
		<ExplorerContextProvider explorer={explorer}>
			<SearchContextProvider search={search}>
				<TopBarPortal
					center={<SearchBar />}
					left={
						<div className="flex flex-row items-center gap-2">
							<span className="truncate text-sm font-medium">{t('labels')}</span>
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
						message={t('no_labels')}
					/>
				}
			/>
		</ExplorerContextProvider>
	);
}
