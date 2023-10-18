import { Info } from '@phosphor-icons/react';
import { getIcon, iconNames } from '@sd/assets/util';
import { useCallback, useEffect, useMemo } from 'react';
import { useDebouncedCallback } from 'use-debounce';
import { stringify } from 'uuid';
import {
	arraysEqual,
	ExplorerSettings,
	FilePathFilterArgs,
	FilePathOrder,
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
import { TopBarPortal } from '../TopBar/Portal';
import LocationOptions from './LocationOptions';

export const Component = () => {
	const [{ path }] = useExplorerSearchParams();
	const { id: locationId } = useZodRouteParams(LocationIdParamsSchema);
	const location = useLibraryQuery(['locations.get', locationId]);
	const rspc = useRspcLibraryContext();

	const onlineLocations = useOnlineLocations();

	const locationOnline = useMemo(() => {
		const pub_id = location.data?.pub_id;
		if (!pub_id) return false;
		return onlineLocations.some((l) => arraysEqual(pub_id, l));
	}, [location.data?.pub_id, onlineLocations]);

	const preferences = useLibraryQuery(['preferences.get']);
	const updatePreferences = useLibraryMutation('preferences.update');

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

	const onSettingsChanged = useDebouncedCallback(
		async (settings: ExplorerSettings<FilePathOrder>) => {
			if (!location.data) return;
			const pubId = stringify(location.data.pub_id);
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
		location: location.data
	});

	const { items, count, loadMore, query } = useItems({ locationId, settings: explorerSettings });

	const explorer = useExplorer({
		items,
		count,
		loadMore,
		isFetchingNextPage: query.isFetchingNextPage,
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
				right={<DefaultTopBarOptions />}
			/>
			<Explorer
				emptyNotice={
					<EmptyNotice
						loading={location.isFetching}
						icon={<img className="h-32 w-32" src={getIcon(iconNames.FolderNoSpace)} />}
						message="No files found here"
					/>
				}
			/>
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
