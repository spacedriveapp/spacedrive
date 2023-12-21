import { memo, useMemo } from 'react';
import { ObjectKindEnum, ObjectOrder, useCache, useLibraryQuery, useNodes } from '@sd/client';
import { LocationIdParamsSchema } from '~/app/route-schemas';
import { Icon } from '~/components';
import { useRouteTitle, useZodRouteParams } from '~/hooks';

import Explorer from '../Explorer';
import { ExplorerContextProvider } from '../Explorer/Context';
import { useObjectsExplorerQuery } from '../Explorer/queries/useObjectsExplorerQuery';
import { createDefaultExplorerSettings, objectOrderingKeysSchema } from '../Explorer/store';
import { DefaultTopBarOptions } from '../Explorer/TopBarOptions';
import { useExplorer, useExplorerSettings } from '../Explorer/useExplorer';
import { EmptyNotice } from '../Explorer/View/EmptyNotice';
import { SearchContextProvider, SearchOptions, useSearch } from '../search';
import SearchBar from '../search/SearchBar';
import { TopBarPortal } from '../TopBar/Portal';

export function Component() {
	const { id: tagId } = useZodRouteParams(LocationIdParamsSchema);
	const result = useLibraryQuery(['tags.get', tagId], { suspense: true });
	useNodes(result.data?.nodes);
	const tag = useCache(result.data?.item);

	useRouteTitle(tag!.name ?? 'Tag');

	const explorerSettings = useExplorerSettings({
		settings: useMemo(() => {
			return createDefaultExplorerSettings<ObjectOrder>({ order: null });
		}, []),
		orderingKeys: objectOrderingKeysSchema
	});

	const explorerSettingsSnapshot = explorerSettings.useSettingsSnapshot();

	const fixedFilters = useMemo(
		() => [
			{ object: { tags: { in: [tag!.id] } } },
			...(explorerSettingsSnapshot.layoutMode === 'media'
				? [{ object: { kind: { in: [ObjectKindEnum.Image, ObjectKindEnum.Video] } } }]
				: [])
		],
		[tag, explorerSettingsSnapshot.layoutMode]
	);

	const search = useSearch({
		fixedFilters
	});

	const objects = useObjectsExplorerQuery({
		arg: { take: 100, filters: search.allFilters },
		explorerSettings
	});

	const explorer = useExplorer({
		...objects,
		isFetchingNextPage: objects.query.isFetchingNextPage,
		settings: explorerSettings,
		parent: { type: 'Tag', tag: tag! }
	});

	return (
		<ExplorerContextProvider explorer={explorer}>
			<SearchContextProvider search={search}>
				<TopBarPortal
					center={<SearchBar />}
					left={
						<div className="flex flex-row items-center gap-2">
							<div
								className="h-[14px] w-[14px] shrink-0 rounded-full"
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
						message="No items assigned to this tag."
					/>
				}
			/>
		</ExplorerContextProvider>
	);
}
