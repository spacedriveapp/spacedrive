import { ArrowClockwise, Info } from '@phosphor-icons/react';
import { useCallback, useEffect, useMemo } from 'react';
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
import { Loader, Tooltip } from '@sd/ui';
import { LocationIdParamsSchema } from '~/app/route-schemas';
import { Folder, Icon } from '~/components';
import {
	useIsLocationIndexing,
	useKeyDeleteFile,
	useRouteTitle,
	useShortcut,
	useZodRouteParams
} from '~/hooks';
import { useQuickRescan } from '~/hooks/useQuickRescan';

import Explorer from '../Explorer';
import { ExplorerContextProvider } from '../Explorer/Context';
import { usePathsInfiniteQuery } from '../Explorer/queries';
import { createDefaultExplorerSettings, filePathOrderingKeysSchema } from '../Explorer/store';
import { DefaultTopBarOptions } from '../Explorer/TopBarOptions';
import { useExplorer, UseExplorerSettings, useExplorerSettings } from '../Explorer/useExplorer';
import { useExplorerSearchParams } from '../Explorer/util';
import { EmptyNotice } from '../Explorer/View';
import { TopBarPortal } from '../TopBar/Portal';
import { TOP_BAR_ICON_STYLE } from '../TopBar/TopBarOptions';
import LocationOptions from './LocationOptions';

export const Component = () => {
	const rspc = useRspcLibraryContext();

	const [{ path }] = useExplorerSearchParams();
	const { id: locationId } = useZodRouteParams(LocationIdParamsSchema);

	const location = useLibraryQuery(['locations.get', locationId]);
	const onlineLocations = useOnlineLocations();

	const rescan = useQuickRescan();

	const locationOnline = useMemo(() => {
		const pub_id = location.data?.pub_id;
		if (!pub_id) return false;
		return onlineLocations.some((l) => arraysEqual(pub_id, l));
	}, [location.data?.pub_id, onlineLocations]);

	const preferences = useLibraryQuery(['preferences.get']);
	const updatePreferences = useLibraryMutation('preferences.update');

	const isLocationIndexing = useIsLocationIndexing(locationId);

	const settings = useMemo(() => {
		const defaults = createDefaultExplorerSettings<FilePathOrder>({
			order: { field: 'name', value: 'Asc' }
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

	const onSettingsChanged = async (
		settings: ExplorerSettings<FilePathOrder>,
		location: Location
	) => {
		if (location.id === locationId && preferences.isLoading) return;

		const pubId = stringify(location.pub_id);

		try {
			await updatePreferences.mutateAsync({
				location: { [pubId]: { explorer: settings } }
			});
			rspc.queryClient.invalidateQueries(['preferences.get']);
		} catch (e) {
			alert('An error has occurred while updating your preferences.');
		}
	};

	const explorerSettings = useExplorerSettings({
		settings,
		onSettingsChanged,
		orderingKeys: filePathOrderingKeysSchema,
		location: location.data
	});

	const { items, count, loadMore, query } = useItems({ locationId, settings: explorerSettings });

	const explorer = useExplorer({
		items,
		count,
		loadMore,
		isFetchingNextPage: query.isFetchingNextPage,
		isLoadingPreferences: preferences.isLoading,
		settings: explorerSettings,
		...(location.data && {
			parent: { type: 'Location', location: location.data }
		})
	});

	useLibrarySubscription(
		['locations.quickRescan', { sub_path: path ?? '', location_id: locationId }],
		{ onData() {} }
	);

	useEffect(() => {
		// Using .call to silence eslint exhaustive deps warning.
		// If clearSelectedItems referenced 'this' then this wouldn't work
		explorer.resetSelectedItems.call(undefined);
	}, [explorer.resetSelectedItems, path]);

	useEffect(() => explorer.scrollRef.current?.scrollTo({ top: 0 }), [explorer.scrollRef, path]);

	useKeyDeleteFile(explorer.selectedItems, location.data?.id);

	useShortcut('rescan', () => rescan(locationId));

	const title = useRouteTitle(
		(path && path?.length > 1 ? getLastSectionOfPath(path) : location.data?.name) ?? ''
	);

	return (
		<ExplorerContextProvider explorer={explorer}>
			<TopBarPortal
				left={
					<div className="flex items-center gap-2">
						<Folder size={22} className="mt-[-1px]" />
						<span className="truncate text-sm font-medium">{title}</span>
						{!locationOnline && (
							<Tooltip label="Location is offline, you can still browse and organize.">
								<Info className="text-ink-faint" />
							</Tooltip>
						)}
						{location.data && (
							<LocationOptions location={location.data} path={path || ''} />
						)}
					</div>
				}
				right={
					<DefaultTopBarOptions
						options={[
							{
								toolTipLabel: 'Reload',
								onClick: () => rescan(locationId),
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
					emptyNotice={
						<EmptyNotice
							loading={location.isFetching}
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
	locationId,
	settings
}: {
	locationId: number;
	settings: UseExplorerSettings<FilePathOrder>;
}) => {
	const [{ path, take }] = useExplorerSearchParams();

	const { library } = useLibraryContext();

	const explorerSettings = settings.useSettingsSnapshot();

	const filter: FilePathFilterArgs = { locationId, path: path ?? '' };

	if (explorerSettings.layoutMode === 'media') {
		filter.object = { kind: [ObjectKindEnum.Image, ObjectKindEnum.Video] };

		if (explorerSettings.mediaViewWithDescendants) filter.withDescendants = true;
	}

	if (!explorerSettings.showHiddenFiles) filter.hidden = false;

	const count = useLibraryQuery(['search.pathsCount', { filter }]);

	const query = usePathsInfiniteQuery({
		arg: { filter, take },
		library,
		settings
	});

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
