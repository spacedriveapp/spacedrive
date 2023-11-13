import { useCallback, useMemo } from 'react';
import { ObjectFilterArgs, ObjectOrder, useLibraryContext, useLibraryQuery } from '@sd/client';
import { LocationIdParamsSchema } from '~/app/route-schemas';
import { Icon } from '~/components';
import { useZodRouteParams } from '~/hooks';

import Explorer from '../Explorer';
import { ExplorerContextProvider } from '../Explorer/Context';
import { useObjectsInfiniteQuery } from '../Explorer/queries';
import { createDefaultExplorerSettings, objectOrderingKeysSchema } from '../Explorer/store';
import { DefaultTopBarOptions } from '../Explorer/TopBarOptions';
import { useExplorer, UseExplorerSettings, useExplorerSettings } from '../Explorer/useExplorer';
import { EmptyNotice } from '../Explorer/View';
import { TopBarPortal } from '../TopBar/Portal';

export const Component = () => {
	const { id: tagId } = useZodRouteParams(LocationIdParamsSchema);
	const tag = useLibraryQuery(['tags.get', tagId], { suspense: true });

	const explorerSettings = useExplorerSettings({
		settings: useMemo(
			() =>
				createDefaultExplorerSettings<ObjectOrder>({
					order: null
				}),
			[]
		),
		orderingKeys: objectOrderingKeysSchema
	});

	const { items, count, loadMore, query } = useItems({ tagId, settings: explorerSettings });

	const explorer = useExplorer({
		items,
		count,
		loadMore,
		settings: explorerSettings,
		...(tag.data && {
			parent: { type: 'Tag', tag: tag.data }
		})
	});

	return (
		<ExplorerContextProvider explorer={explorer}>
			<TopBarPortal right={<DefaultTopBarOptions />} />
			<Explorer
				emptyNotice={
					<EmptyNotice
						loading={query.isFetching}
						icon={<Icon name="Tags" size={128} />}
						message="No items assigned to this tag."
					/>
				}
			/>
		</ExplorerContextProvider>
	);
};

function useItems({
	tagId,
	settings
}: {
	tagId: number;
	settings: UseExplorerSettings<ObjectOrder>;
}) {
	const { library } = useLibraryContext();

	const filter: ObjectFilterArgs = { tags: [tagId] };

	const count = useLibraryQuery(['search.objectsCount', { filter }]);

	const query = useObjectsInfiniteQuery({
		library,
		arg: { take: 100, filter: { tags: [tagId] } },
		settings
	});

	const items = useMemo(() => query.data?.pages?.flatMap((d) => d.items) ?? null, [query.data]);

	const loadMore = useCallback(() => {
		if (query.hasNextPage && !query.isFetchingNextPage) {
			query.fetchNextPage.call(undefined);
		}
	}, [query.hasNextPage, query.isFetchingNextPage, query.fetchNextPage]);

	return { query, items, loadMore, count: count.data };
}
