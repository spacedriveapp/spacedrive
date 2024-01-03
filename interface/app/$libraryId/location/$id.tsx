import { ArrowClockwise, Info } from '@phosphor-icons/react';
import { memo, useEffect, useMemo } from 'react';
import { useSearchParams as useRawSearchParams } from 'react-router-dom';
import { stringify } from 'uuid';
import {
	arraysEqual,
	ExplorerSettings,
	FilePathOrder,
	Location,
	ObjectKindEnum,
	useCache,
	useLibraryMutation,
	useLibraryQuery,
	useLibrarySubscription,
	useNodes,
	useOnlineLocations,
	useRspcLibraryContext
} from '@sd/client';
import { Loader, Tooltip } from '@sd/ui';
import { LocationIdParamsSchema } from '~/app/route-schemas';
import { Folder, Icon } from '~/components';
import {
	useIsLocationIndexing,
	useKeyDeleteFile,
	useLocale,
	useRouteTitle,
	useShortcut,
	useZodRouteParams
} from '~/hooks';
import { useQuickRescan } from '~/hooks/useQuickRescan';

import Explorer from '../Explorer';
import { ExplorerContextProvider } from '../Explorer/Context';
import { usePathsExplorerQuery } from '../Explorer/queries';
import { createDefaultExplorerSettings, filePathOrderingKeysSchema } from '../Explorer/store';
import { DefaultTopBarOptions } from '../Explorer/TopBarOptions';
import { useExplorer, UseExplorerSettings, useExplorerSettings } from '../Explorer/useExplorer';
import { useExplorerSearchParams } from '../Explorer/util';
import { EmptyNotice } from '../Explorer/View/EmptyNotice';
import SearchOptions, { SearchContextProvider, useSearch } from '../Search';
import SearchBar from '../Search/SearchBar';
import { TopBarPortal } from '../TopBar/Portal';
import { TOP_BAR_ICON_STYLE } from '../TopBar/TopBarOptions';
import LocationOptions from './LocationOptions';

export const Component = () => {
	const { id: locationId } = useZodRouteParams(LocationIdParamsSchema);
	const [{ path }] = useExplorerSearchParams();
	const result = useLibraryQuery(['locations.get', locationId], {
		keepPreviousData: true,
		suspense: true
	});
	useNodes(result.data?.nodes);
	const location = useCache(result.data?.item);

	// 'key' allows search state to be thrown out when entering a folder
	return <LocationExplorer key={path} location={location!} />;
};

const LocationExplorer = ({ location }: { location: Location; path?: string }) => {
	const [{ path, take }] = useExplorerSearchParams();

	const onlineLocations = useOnlineLocations();

	const rescan = useQuickRescan();

	const locationOnline = useMemo(() => {
		const pub_id = location.pub_id;
		if (!pub_id) return false;
		return onlineLocations.some((l) => arraysEqual(pub_id, l));
	}, [location.pub_id, onlineLocations]);

	const { explorerSettings, preferences } = useLocationExplorerSettings(location);

	const { layoutMode, mediaViewWithDescendants, showHiddenFiles } =
		explorerSettings.useSettingsSnapshot();

	const search = useLocationSearch(explorerSettings, location);

	const paths = usePathsExplorerQuery({
		arg: {
			filters: [
				...search.allFilters,
				{
					filePath: {
						path: {
							location_id: location.id,
							path: path ?? '',
							include_descendants:
								search.search !== '' ||
								search.dynamicFilters.length > 0 ||
								(layoutMode === 'media' && mediaViewWithDescendants)
						}
					}
				},
				!showHiddenFiles && { filePath: { hidden: false } }
			].filter(Boolean) as any,
			take
		},
		explorerSettings
	});

	const explorer = useExplorer({
		...paths,
		isFetchingNextPage: paths.query.isFetchingNextPage,
		isLoadingPreferences: preferences.isLoading,
		settings: explorerSettings,
		parent: { type: 'Location', location }
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

	const title = useRouteTitle(
		(path && path?.length > 1 ? getLastSectionOfPath(path) : location.name) ?? ''
	);

	const isLocationIndexing = useIsLocationIndexing(location.id);

	const { t } = useLocale();

	return (
		<ExplorerContextProvider explorer={explorer}>
			<SearchContextProvider search={search}>
				<TopBarPortal
					center={<SearchBar />}
					left={
						<div className="flex items-center gap-2">
							<Folder size={22} className="mt-[-1px]" />
							<span className="truncate text-sm font-medium">{title}</span>
							{!locationOnline && (
								<Tooltip label={t('location_offline_tooltip')}>
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
									toolTipLabel: t('reload'),
									onClick: () => rescan(location.id),
									icon: <ArrowClockwise className={TOP_BAR_ICON_STYLE} />,
									individual: true,
									showAtResolution: 'xl:flex'
								}
							]}
						/>
					}
				>
					{search.open && (
						<>
							<hr className="w-full border-t border-sidebar-divider bg-sidebar-divider" />
							<SearchOptions />
						</>
					)}
				</TopBarPortal>
			</SearchContextProvider>
			{isLocationIndexing ? (
				<div className="flex h-full w-full items-center justify-center">
					<Loader />
				</div>
			) : !preferences.isLoading ? (
				<Explorer
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

function getLastSectionOfPath(path: string): string | undefined {
	if (path.endsWith('/')) {
		path = path.slice(0, -1);
	}
	const sections = path.split('/');
	const lastSection = sections[sections.length - 1];
	return lastSection;
}

function useLocationExplorerSettings(location: Location) {
	const rspc = useRspcLibraryContext();

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

	const onSettingsChanged = async (
		settings: ExplorerSettings<FilePathOrder>,
		changedLocation: Location
	) => {
		if (changedLocation.id === location.id && preferences.isLoading) return;

		const pubId = stringify(changedLocation.pub_id);

		try {
			await updatePreferences.mutateAsync({
				location: { [pubId]: { explorer: settings } }
			});
			rspc.queryClient.invalidateQueries(['preferences.get']);
		} catch (e) {
			alert('An error has occurred while updating your preferences.');
		}
	};

	return {
		explorerSettings: useExplorerSettings({
			settings,
			onSettingsChanged,
			orderingKeys: filePathOrderingKeysSchema,
			location
		}),
		preferences
	};
}

function useLocationSearch(
	explorerSettings: UseExplorerSettings<FilePathOrder>,
	location: Location
) {
	const [searchParams, setSearchParams] = useRawSearchParams();
	const explorerSettingsSnapshot = explorerSettings.useSettingsSnapshot();

	const fixedFilters = useMemo(
		() => [
			{ filePath: { locations: { in: [location.id] } } },
			...(explorerSettingsSnapshot.layoutMode === 'media'
				? [{ object: { kind: { in: [ObjectKindEnum.Image, ObjectKindEnum.Video] } } }]
				: [])
		],
		[location.id, explorerSettingsSnapshot.layoutMode]
	);

	const filtersParam = searchParams.get('filters');
	const dynamicFilters = useMemo(() => JSON.parse(filtersParam ?? '[]'), [filtersParam]);

	const searchQueryParam = searchParams.get('search');

	const search = useSearch({
		open: !!searchQueryParam || dynamicFilters.length > 0 || undefined,
		search: searchParams.get('search') ?? undefined,
		fixedFilters,
		dynamicFilters
	});

	useEffect(() => {
		setSearchParams(
			(p) => {
				if (search.dynamicFilters.length > 0)
					p.set('filters', JSON.stringify(search.dynamicFilters));
				else p.delete('filters');

				return p;
			},
			{ replace: true }
		);
	}, [search.dynamicFilters, setSearchParams]);

	const searchQuery = search.search;

	useEffect(() => {
		setSearchParams(
			(p) => {
				if (searchQuery !== '') p.set('search', searchQuery);
				else p.delete('search');

				return p;
			},
			{ replace: true }
		);
	}, [searchQuery, setSearchParams]);

	return search;
}
