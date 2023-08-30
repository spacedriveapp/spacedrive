import { useInfiniteQuery, useQueryClient } from '@tanstack/react-query';
import { useCallback, useEffect, useMemo } from 'react';
import { useDebouncedCallback } from 'use-debounce';
import { stringify } from 'uuid';
import {
	ExplorerItem,
	ExplorerSettings,
	FilePathCursorOrdering,
	FilePathFilterArgs,
	FilePathOrderAndPaginationArgs,
	FilePathSearchArgs,
	FilePathSearchOrdering,
	ObjectCursorOrdering,
	SearchData,
	useLibraryContext,
	useLibraryMutation,
	useLibraryQuery,
	useLibrarySubscription,
	useRspcLibraryContext
} from '@sd/client';
import { LocationIdParamsSchema } from '~/app/route-schemas';
import { Folder } from '~/components';
import { useKeyDeleteFile, useZodRouteParams } from '~/hooks';
import Explorer from '../Explorer';
import { ExplorerContextProvider } from '../Explorer/Context';
import { DefaultTopBarOptions } from '../Explorer/TopBarOptions';
import {
	createDefaultExplorerSettings,
	filePathOrderingKeysSchema,
	getExplorerStore
} from '../Explorer/store';
import { UseExplorerSettings, useExplorer, useExplorerSettings } from '../Explorer/useExplorer';
import { useExplorerSearchParams } from '../Explorer/util';
import { TopBarPortal } from '../TopBar/Portal';
import LocationOptions from './LocationOptions';

export const Component = () => {
	const [{ path }] = useExplorerSearchParams();
	const queryClient = useQueryClient();
	const { id: locationId } = useZodRouteParams(LocationIdParamsSchema);
	const location = useLibraryQuery(['locations.get', locationId]);

	const preferences = useLibraryQuery(['preferences.get']);
	const updatePreferences = useLibraryMutation('preferences.update');

	const settings = useMemo(() => {
		const defaults = createDefaultExplorerSettings<FilePathSearchOrdering>({
			order: {
				field: 'name',
				value: 'Asc'
			}
		});

		if (!location.data) return defaults;

		const pubId = stringify(location.data.pub_id);

		const settings = preferences.data?.location?.[pubId]?.explorer;

		if (!settings) return defaults;

		for (const [key, value] of Object.entries(settings)) {
			if (value !== null) Object.assign(defaults, { [key]: value });
		}

		return defaults;
	}, [location.data, preferences.data?.location]);

	const onSettingsChanged = useDebouncedCallback(
		async (settings: ExplorerSettings<FilePathSearchOrdering>) => {
			if (!location.data) return;
			const pubId = stringify(location.data.pub_id);
			try {
				await updatePreferences.mutateAsync({
					location: {
						[pubId]: {
							explorer: settings
						}
					}
				});
				queryClient.invalidateQueries(['preferences.get']);
			} catch (e) {
				alert('An error has occurred while updating your preferences.');
			}
		},
		500
	);

	const explorerSettings = useExplorerSettings<FilePathSearchOrdering>({
		settings,
		onSettingsChanged,
		orderingKeys: filePathOrderingKeysSchema
	});

	const { items, count, loadMore } = useItems({ locationId, settings: explorerSettings });

	const explorer = useExplorer({
		items,
		count,
		loadMore,
		parent: location.data
			? {
					type: 'Location',
					location: location.data
			  }
			: undefined,
		settings: explorerSettings
	});

	useLibrarySubscription(
		[
			'locations.quickRescan',
			{
				sub_path: path ?? '',
				location_id: locationId
			}
		],
		{ onData() {} }
	);

	useEffect(() => {
		// Using .call to silence eslint exhaustive deps warning.
		// If clearSelectedItems referenced 'this' then this wouldn't work
		explorer.resetSelectedItems.call(undefined);
	}, [explorer.resetSelectedItems, path]);

	useKeyDeleteFile(explorer.selectedItems, location.data?.id);

	useEffect(() => explorer.scrollRef.current?.scrollTo({ top: 0 }), [explorer.scrollRef, path]);

	return (
		<ExplorerContextProvider explorer={explorer}>
			<TopBarPortal
				left={
					<div className="flex items-center gap-2">
						<Folder size={22} className="mt-[-1px]" />
						<span className="truncate text-sm font-medium">
							{path && path?.length > 1
								? getLastSectionOfPath(path)
								: location.data?.name}
						</span>
						{location.data && (
							<LocationOptions location={location.data} path={path || ''} />
						)}
					</div>
				}
				right={<DefaultTopBarOptions />}
			/>

			<Explorer />
		</ExplorerContextProvider>
	);
};

const useItems = ({
	locationId,
	settings
}: {
	locationId: number;
	settings: UseExplorerSettings<FilePathSearchOrdering>;
}) => {
	const [{ path, take }] = useExplorerSearchParams();

	const ctx = useRspcLibraryContext();
	const { library } = useLibraryContext();

	const explorerSettings = settings.useSettingsSnapshot();

	const filter: FilePathFilterArgs = {
		locationId,
		...(explorerSettings.layoutMode === 'media'
			? { object: { kind: [5, 7] } }
			: { path: path ?? '' })
	};

	const count = useLibraryQuery(['search.pathsCount', { filter }]);

	const arg: FilePathSearchArgs = {
		filter,
		take,
		orderAndPagination: explorerSettings.order
			? {
					orderOnly: explorerSettings.order
			  }
			: undefined
	};

	const query = useInfiniteQuery({
		queryKey: ['search.paths', { library_id: library.uuid, arg }] as const,
		queryFn: ({ pageParam, queryKey }) => {
			const c: SearchData<ExplorerItem> | undefined = pageParam;
			const { order } = explorerSettings;

			let orderAndPagination: FilePathOrderAndPaginationArgs | undefined;

			if (!c) {
				if (order) orderAndPagination = { orderOnly: order };
			} else {
				let cursor: FilePathCursorOrdering | undefined;

				const cItem = c.items[c.items.length - 1]! as Extract<
					ExplorerItem,
					{ type: 'Path' }
				>;

				if (!order) cursor = { none: [] };
				else {
					switch (order.field) {
						case 'name': {
							const data = cItem.item.name;
							if (data !== null)
								cursor = {
									name: {
										order: order.value,
										data
									}
								};
							break;
						}
						case 'dateCreated': {
							const data = cItem.item.date_created;
							if (data !== null)
								cursor = {
									dateCreated: {
										order: order.value,
										data
									}
								};
							break;
						}
						case 'dateModified': {
							const data = cItem.item.date_modified;
							if (data !== null)
								cursor = {
									dateModified: {
										order: order.value,
										data
									}
								};
							break;
						}
						case 'dateIndexed': {
							const data = cItem.item.date_indexed;
							if (data !== null)
								cursor = {
									dateIndexed: {
										order: order.value,
										data
									}
								};
							break;
						}
						case 'object': {
							const object = cItem.item.object;
							if (!object) break;

							let objectCursor: ObjectCursorOrdering | undefined;

							switch (order.value.field) {
								case 'dateAccessed': {
									const data = object.date_accessed;
									if (data !== null)
										objectCursor = {
											dateAccessed: {
												order: order.value.value,
												data
											}
										};
									break;
								}
								case 'kind': {
									const data = object.kind;
									if (data !== null)
										objectCursor = {
											kind: {
												order: order.value.value,
												data
											}
										};
									break;
								}
							}

							if (objectCursor)
								cursor = {
									object: objectCursor
								};

							break;
						}
					}
				}

				if (cursor) orderAndPagination = { cursor };
			}

			return ctx.client.query([
				'search.paths',
				{
					...queryKey[1].arg,
					orderAndPagination
				}
			]);
		},
		getNextPageParam: (lastPage) => lastPage,
		keepPreviousData: true,
		onSuccess: () => getExplorerStore().resetNewThumbnails()
	});

	const items = useMemo(() => query.data?.pages.flatMap((d) => d.items) || null, [query.data]);

	const loadMore = useCallback(() => {
		if (query.hasNextPage && !query.isFetchingNextPage) {
			query.fetchNextPage.call(undefined);
		}
	}, [query.hasNextPage, query.isFetchingNextPage, query.fetchNextPage]);

	return {
		query,
		items,
		loadMore,
		count: count.data
	};
};

function getLastSectionOfPath(path: string): string | undefined {
	if (path.endsWith('/')) {
		path = path.slice(0, -1);
	}
	const sections = path.split('/');
	const lastSection = sections[sections.length - 1];
	return lastSection;
}
