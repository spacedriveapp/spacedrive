import { ArrowClockwise, Info } from '@phosphor-icons/react';
import { Suspense, useCallback, useEffect, useMemo } from 'react';
import { useDebouncedCallback } from 'use-debounce';
import { stringify } from 'uuid';
import {
	arraysEqual,
	ExplorerSettings,
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
import { Loader, Tooltip } from '@sd/ui';
import { LocationIdParamsSchema } from '~/app/route-schemas';
import { Folder, Icon } from '~/components';
import { useIsLocationIndexing, useKeyDeleteFile, useShortcut, useZodRouteParams } from '~/hooks';
import { useQuickRescan } from '~/hooks/useQuickRescan';

import Explorer from '../Explorer';
import { ExplorerContextProvider } from '../Explorer/Context';
import { usePathsInfiniteQuery } from '../Explorer/queries';
import { SearchContextProvider } from '../Explorer/Search/Context';
import { useSearchFilters } from '../Explorer/Search/store';
import { createDefaultExplorerSettings, filePathOrderingKeysSchema } from '../Explorer/store';
import { DefaultTopBarOptions } from '../Explorer/TopBarOptions';
import { useExplorer, UseExplorerSettings, useExplorerSettings } from '../Explorer/useExplorer';
import { useExplorerSearchParams } from '../Explorer/util';
import { EmptyNotice } from '../Explorer/View';
import { TopBarPortal } from '../TopBar/Portal';
import { TOP_BAR_ICON_STYLE } from '../TopBar/TopBarOptions';
import LocationOptions from './LocationOptions';

export const Component = () => {
	const [{ path }] = useExplorerSearchParams();
	const { id: locationId } = useZodRouteParams(LocationIdParamsSchema);
	const location = useLibraryQuery(['locations.get', locationId], {
		keepPreviousData: true,
		suspense: true
	});

	return (
		<SearchContextProvider>
			<Suspense>
				<LocationExplorer path={path} location={location.data!} />)
			</Suspense>
		</SearchContextProvider>
	);
};

const LocationExplorer = ({ location, path }: { location: Location; path?: string }) => {
	const rspc = useRspcLibraryContext();

	const onlineLocations = useOnlineLocations();

	const rescan = useQuickRescan();

	const locationOnline = useMemo(() => {
		const pub_id = location?.pub_id;
		if (!pub_id) return false;
		return onlineLocations.some((l) => arraysEqual(pub_id, l));
	}, [location?.pub_id, onlineLocations]);

	const preferences = useLibraryQuery(['preferences.get']);
	const updatePreferences = useLibraryMutation('preferences.update');

	const isLocationIndexing = useIsLocationIndexing(location.id);

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
			if (preferences.isLoading) return;

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
		isLoadingPreferences: preferences.isLoading,
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

	useEffect(() => explorer.scrollRef.current?.scrollTo({ top: 0 }), [explorer.scrollRef, path]);

	useKeyDeleteFile(explorer.selectedItems, location.id);

	useShortcut('rescan', () => rescan(location.id));

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
				right={
					<DefaultTopBarOptions
						options={[
							{
								toolTipLabel: 'Reload',
								onClick: () => rescan(location.id),
								icon: <ArrowClockwise className={TOP_BAR_ICON_STYLE} />,
								individual: true,
								showAtResolution: 'xl:flex'
							}
						]}
					/>
				}
			/>
			{isLocationIndexing ? (
				<div className="flex h-full w-full items-center justify-center">
					<Loader />
				</div>
			) : !preferences.isLoading ? (
				<Explorer
					showFilterBar
					emptyNotice={
						<EmptyNotice
							icon={<Icon name="FolderNoSpace" size={128} />}
							message="No files found here"
						/>
					}
				/>
			) : null}
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

	// useMemo lets us embrace immutability and use fixedFilters in useEffects!
	const fixedFilters = useMemo(
		() => [
			{ filePath: { locations: { in: [location.id] } } },
			...(explorerSettings.layoutMode === 'media'
				? [{ object: { kind: { in: [ObjectKindEnum.Image, ObjectKindEnum.Video] } } }]
				: [])
		],
		[location.id, explorerSettings.layoutMode]
	);

	const baseFilters = useSearchFilters('paths', fixedFilters);

	const filters = [...baseFilters];

	filters.push({
		filePath: {
			path: {
				location_id: location.id,
				path: path ?? '',
				include_descendants:
					explorerSettings.layoutMode === 'media' &&
					explorerSettings.mediaViewWithDescendants
			}
		}
	});

	if (!explorerSettings.showHiddenFiles) filters.push({ filePath: { hidden: false } });

	const query = usePathsInfiniteQuery({
		arg: { filters, take },
		library,
		settings
	});

	const count = useLibraryQuery(['search.pathsCount', { filters }], { enabled: query.isSuccess });

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
