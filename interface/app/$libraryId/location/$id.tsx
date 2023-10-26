import { Info } from '@phosphor-icons/react';
import { getIcon, iconNames } from '@sd/assets/util';
import { Suspense, useCallback, useEffect, useMemo } from 'react';
import { useDebouncedCallback } from 'use-debounce';
import { stringify } from 'uuid';
import {
	arraysEqual,
	ExplorerSettings,
	FilePathFilterArgs,
	FilePathOrder,
	Location,
	ObjectKindEnum,
	useLibraryContext,
	useLibraryMutation,
	useLibraryQuery,
	useLibrarySubscription,
	useOnlineLocations,
	useRspcLibraryContext
} from '@sd/client';
import { Tooltip } from '@sd/ui';
import { LocationIdParamsSchema } from '~/app/route-schemas';
import { Folder } from '~/components';
import { useKeyDeleteFile, useZodRouteParams } from '~/hooks';

import Explorer from '../Explorer';
import { ExplorerContextProvider } from '../Explorer/Context';
import { usePathsInfiniteQuery } from '../Explorer/queries';
import { createDefaultExplorerSettings, filePathOrderingKeysSchema } from '../Explorer/store';
import { DefaultTopBarOptions } from '../Explorer/TopBarOptions';
import { useExplorer, UseExplorerSettings, useExplorerSettings } from '../Explorer/useExplorer';
import { useExplorerSearchParams } from '../Explorer/util';
import { EmptyNotice } from '../Explorer/View';
import SearchOptions from '../Explorer/View/SearchOptions';
import { FilterType } from '../Explorer/View/SearchOptions/Filters';
import { useSearchFilters } from '../Explorer/View/SearchOptions/store';
import { inOrNotIn } from '../Explorer/View/SearchOptions/util';
import { TopBarPortal } from '../TopBar/Portal';
import LocationOptions from './LocationOptions';

export const Component = () => {
	const [{ path }] = useExplorerSearchParams();
	const { id: locationId } = useZodRouteParams(LocationIdParamsSchema);
	const location = useLibraryQuery(['locations.get', locationId], {
		keepPreviousData: true,
		suspense: true
	});

	return (
		<Suspense>
			<LocationExplorer path={path} location={location.data!} />)
		</Suspense>
	);
};

const LocationExplorer = ({ location, path }: { location: Location; path?: string }) => {
	const rspc = useRspcLibraryContext();

	const onlineLocations = useOnlineLocations();

	const locationOnline = useMemo(() => {
		const pub_id = location?.pub_id;
		if (!pub_id) return false;
		return onlineLocations.some((l) => arraysEqual(pub_id, l));
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [location?.pub_id, onlineLocations]);

	const preferences = useLibraryQuery(['preferences.get']);
	const updatePreferences = useLibraryMutation('preferences.update');

	const settings = useMemo(() => {
		const defaults = createDefaultExplorerSettings<FilePathOrder>({
			order: { field: 'name', value: 'Asc' }
		});

		if (!location) return defaults;

		const pubId = stringify(location.pub_id);

		const settings = preferences.data?.location?.[pubId]?.explorer;

		if (!settings) return defaults;

		for (const [key, value] of Object.entries(settings)) {
			if (value !== null) Object.assign(defaults, { [key]: value });
		}

		return defaults;
	}, [location, preferences.data?.location]);

	const onSettingsChanged = useDebouncedCallback(
		async (settings: ExplorerSettings<FilePathOrder>) => {
			const pubId = stringify(location.pub_id);
			try {
				await updatePreferences.mutateAsync({
					location: { [pubId]: { explorer: settings } }
				});
				rspc.queryClient.invalidateQueries(['preferences.get']);
			} catch (e) {
				alert('An error has occurred while updating your preferences.');
			}
		},
		500
	);

	const explorerSettings = useExplorerSettings({
		settings,
		onSettingsChanged,
		orderingKeys: filePathOrderingKeysSchema,
		location
	});

	const { items, count, loadMore, query } = useItems({
		location,
		settings: explorerSettings
	});

	const explorer = useExplorer({
		items,
		count,
		loadMore,
		isFetchingNextPage: query.isFetchingNextPage,
		settings: explorerSettings,
		...(location && {
			parent: { type: 'Location', location }
		})
	});

	useLibrarySubscription(
		['locations.quickRescan', { sub_path: path ?? '', location_id: location.id }],
		{ onData() {} }
	);

	useEffect(() => {
		// Using .call to silence eslint exhaustive deps warning.
		// If clearSelectedItems referenced 'this' then this wouldn't work
		explorer.resetSelectedItems.call(undefined);
	}, [explorer.resetSelectedItems, path]);

	useKeyDeleteFile(explorer.selectedItems, location.id);

	useEffect(() => explorer.scrollRef.current?.scrollTo({ top: 0 }), [explorer.scrollRef, path]);

	return (
		<ExplorerContextProvider explorer={explorer}>
			<TopBarPortal
				left={
					<div className="flex items-center gap-2">
						<Folder size={22} className="mt-[-1px]" />
						<span className="truncate text-sm font-medium">
							{path && path?.length > 1 ? getLastSectionOfPath(path) : location.name}
						</span>
						{!locationOnline && (
							<Tooltip label="Location is offline, you can still browse and organize.">
								<Info className="text-ink-faint" />
							</Tooltip>
						)}
						<LocationOptions location={location} path={path || ''} />
					</div>
				}
				right={<DefaultTopBarOptions />}
			/>
			<Explorer
				showFilterBar
				emptyNotice={
					<EmptyNotice
						// loading={location.isFetching}
						icon={<img className="h-32 w-32" src={getIcon(iconNames.FolderNoSpace)} />}
						message="No files found here"
					/>
				}
			/>
		</ExplorerContextProvider>
	);
};

const useItems = ({
	location,
	settings
}: {
	location: Location;
	settings: UseExplorerSettings<FilePathOrder>;
}) => {
	const [{ path, take }] = useExplorerSearchParams();

	const { library } = useLibraryContext();

	const explorerSettings = settings.useSettingsSnapshot();

	const filterArgs = useSearchFilters('paths', [
		{
			name: location.name || '',
			value: location.id.toString(),
			type: FilterType.Location,
			icon: 'Folder'
		},
		...(explorerSettings.layoutMode === 'media'
			? [
					{
						name: 'Image',
						value: ObjectKindEnum.Image,
						type: FilterType.Kind
					},
					{
						name: 'Video',
						value: ObjectKindEnum.Video,
						type: FilterType.Kind
					}
			  ]
			: [])
	]);

	// useEffect(() => {
	// 	console.log({ filterArgs });
	// }, [JSON.stringify(filterArgs)]);

	const filter: FilePathFilterArgs = {
		// locations: { in: [location?.id || 0] },
		...filterArgs,
		path: path ?? ''
	};

	if (explorerSettings.layoutMode === 'media' && explorerSettings.mediaViewWithDescendants)
		filter.withDescendants = true;

	if (!explorerSettings.showHiddenFiles) filter.hidden = false;

	const query = usePathsInfiniteQuery({
		arg: { filter, take },
		library,
		settings
	});

	const count = useLibraryQuery(['search.pathsCount', { filter }], { enabled: query.isSuccess });

	const items = useMemo(() => query.data?.pages.flatMap((d) => d.items) ?? null, [query.data]);

	const loadMore = useCallback(() => {
		if (query.hasNextPage && !query.isFetchingNextPage) {
			query.fetchNextPage.call(undefined);
		}
	}, [query.hasNextPage, query.isFetchingNextPage, query.fetchNextPage]);

	return { query, items, loadMore, count: count.data };
};

function getLastSectionOfPath(path: string): string | undefined {
	if (path.endsWith('/')) {
		path = path.slice(0, -1);
	}
	const sections = path.split('/');
	const lastSection = sections[sections.length - 1];
	return lastSection;
}
