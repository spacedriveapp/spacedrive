import { useInfiniteQuery, useQueryClient } from '@tanstack/react-query';
import { useCallback, useEffect, useMemo } from 'react';
import { useDebouncedCallback } from 'use-debounce';
import { stringify } from 'uuid';
import {
	ExplorerSettings,
	FilePathSearchOrdering,
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

	const { items, loadMore } = useItems({ locationId, settings: explorerSettings });

	const explorer = useExplorer({
		items,
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

	return (
		<ExplorerContextProvider explorer={explorer}>
			<TopBarPortal
				left={
					<div className="group flex flex-row items-center space-x-2">
						<span className="flex flex-row items-center">
							<Folder size={22} className="ml-3 mr-2 mt-[-1px] inline-block" />
							<span className="max-w-[100px] truncate text-sm font-medium">
								{path && path?.length > 1
									? getLastSectionOfPath(path)
									: location.data?.name}
							</span>
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

	const query = useInfiniteQuery({
		queryKey: [
			'search.paths',
			{
				library_id: library.uuid,
				arg: {
					order: explorerSettings.order,
					filter: {
						locationId,
						...(explorerSettings.layoutMode === 'media'
							? { object: { kind: [5, 7] } }
							: { path: path ?? '' })
					},
					take
				}
			}
		] as const,
		queryFn: ({ pageParam: cursor, queryKey }) =>
			ctx.client.query([
				'search.paths',
				{
					...queryKey[1].arg,
					cursor
				}
			]),
		getNextPageParam: (lastPage) => lastPage.cursor ?? undefined,
		keepPreviousData: true,
		onSuccess: () => getExplorerStore().resetNewThumbnails()
	});

	const items = useMemo(() => query.data?.pages.flatMap((d) => d.items) || null, [query.data]);

	const loadMore = useCallback(() => {
		if (query.hasNextPage && !query.isFetchingNextPage) {
			query.fetchNextPage.call(undefined);
		}
	}, [query.hasNextPage, query.isFetchingNextPage, query.fetchNextPage]);

	return { query, items, loadMore };
};

function getLastSectionOfPath(path: string): string | undefined {
	if (path.endsWith('/')) {
		path = path.slice(0, -1);
	}
	const sections = path.split('/');
	const lastSection = sections[sections.length - 1];
	return lastSection;
}
