import { useMemo } from 'react';
import { ObjectOrder, useCache, useLibraryQuery, useNodes } from '@sd/client';
import { LocationIdParamsSchema } from '~/app/route-schemas';
import { Icon } from '~/components';
import { useLocale, useRouteTitle, useZodRouteParams } from '~/hooks';

import Explorer from '../Explorer';
import { ExplorerContextProvider } from '../Explorer/Context';
import { createDefaultExplorerSettings, objectOrderingKeysSchema } from '../Explorer/store';
import { DefaultTopBarOptions } from '../Explorer/TopBarOptions';
import { useExplorer, useExplorerSettings } from '../Explorer/useExplorer';
import { EmptyNotice } from '../Explorer/View/EmptyNotice';
import { SearchContextProvider, SearchOptions, useSearchFromSearchParams } from '../search';
import SearchBar from '../search/SearchBar';
import { useSearchExplorerQuery } from '../search/useSearchExplorerQuery';
import { TopBarPortal } from '../TopBar/Portal';

export function Component() {
	const { id: tagId } = useZodRouteParams(LocationIdParamsSchema);
	const result = useLibraryQuery(['tags.get', tagId], { suspense: true });
	useNodes(result.data?.nodes);
	const tag = useCache(result.data?.item);

	const { t } = useLocale();

	useRouteTitle(tag!.name ?? 'Tag');

	const explorerSettings = useExplorerSettings({
		settings: useMemo(() => {
			return createDefaultExplorerSettings<ObjectOrder>({ order: null });
		}, []),
		orderingKeys: objectOrderingKeysSchema
	});

	const search = useSearchFromSearchParams();

	const defaultFilters = useMemo(() => [{ object: { tags: { in: [tag.id] } } }], [tag.id]);

	const items = useSearchExplorerQuery({
		search,
		explorerSettings,
		filters: search.allFilters.length > 0 ? search.allFilters : defaultFilters,
		take: 100,
		objects: { order: explorerSettings.useSettingsSnapshot().order }
	});

	const explorer = useExplorer({
		...items,
		isFetchingNextPage: items.query.isFetchingNextPage,
		settings: explorerSettings,
		parent: { type: 'Tag', tag: tag! }
	});

	return (
		<ExplorerContextProvider explorer={explorer}>
			<SearchContextProvider search={search}>
				<TopBarPortal
					center={<SearchBar defaultFilters={defaultFilters} defaultTarget="objects" />}
					left={
						<div className="flex flex-row items-center gap-2">
							<div
								className="size-[14px] shrink-0 rounded-full"
								style={{ backgroundColor: tag!.color || '#efefef' }}
							/>
							<span className="truncate text-sm font-medium">{tag?.name}</span>
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
						icon={<Icon name="Tags" size={128} />}
						message={t('tags_notice_message')}
					/>
				}
			/>
		</ExplorerContextProvider>
	);
}
