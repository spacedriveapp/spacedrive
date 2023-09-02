import { iconNames } from '@sd/assets/util';
import { useInfiniteQuery } from '@tanstack/react-query';
import { useMemo } from 'react';
import {
	Category,
	FilePathFilterArgs,
	FilePathSearchArgs,
	FilePathSearchOrdering,
	ObjectFilterArgs,
	ObjectKindEnum,
	ObjectSearchOrdering,
	useLibraryContext,
	useLibraryQuery,
	useRspcLibraryContext
} from '@sd/client';
import { usePathsInfiniteQuery } from '~/hooks/usePathsInfiniteQuery';
import {
	createDefaultExplorerSettings,
	filePathOrderingKeysSchema,
	objectOrderingKeysSchema
} from '../Explorer/store';
import { useExplorer, useExplorerSettings } from '../Explorer/useExplorer';
import { usePageLayoutContext } from '../PageLayout/Context';

export const IconForCategory: Partial<Record<Category, string>> = {
	Recents: iconNames.Collection,
	Favorites: iconNames.Heart,
	Albums: iconNames.Album,
	Photos: iconNames.Image,
	Videos: iconNames.Video,
	Movies: iconNames.Movie,
	Music: iconNames.Audio,
	Documents: iconNames.Document,
	Downloads: iconNames.Package,
	Applications: iconNames.Application,
	Games: iconNames.Game,
	Books: iconNames.Book,
	Encrypted: iconNames.Lock,
	Databases: iconNames.Database,
	Projects: iconNames.Folder,
	Trash: iconNames.Trash
};

export const IconToDescription = {
	Recents: "See files you've recently opened or created",
	Favorites: 'See files you have marked as favorites',
	Albums: 'Organize your photos and videos into albums',
	Photos: 'View all photos in your library',
	Videos: 'View all videos in your library',
	Movies: 'View all movies in your library',
	Music: 'View all music in your library',
	Documents: 'View all documents in your library',
	Downloads: 'View all downloads in your library',
	Encrypted: 'View all encrypted files in your library',
	Projects: 'View all projects in your library',
	Applications: 'View all applications in your library',
	Archives: 'View all archives in your library',
	Databases: 'View all databases in your library',
	Games: 'View all games in your library',
	Books: 'View all books in your library',
	Contacts: 'View all contacts in your library',
	Trash: 'View all files in your trash'
};

export const OBJECT_CATEGORIES: Category[] = ['Recents', 'Favorites'];

// this is a gross function so it's in a separate hook :)
export function useCategoryExplorer(category: Category) {
	const rspc = useRspcLibraryContext();
	const { library } = useLibraryContext();
	const page = usePageLayoutContext();

	const isObjectQuery = OBJECT_CATEGORIES.includes(category);

	const pathsExplorerSettings = useExplorerSettings({
		settings: useMemo(() => createDefaultExplorerSettings<FilePathSearchOrdering>(), []),
		orderingKeys: filePathOrderingKeysSchema
	});

	const objectsExplorerSettings = useExplorerSettings({
		settings: useMemo(() => createDefaultExplorerSettings<ObjectSearchOrdering>(), []),
		orderingKeys: objectOrderingKeysSchema
	});

	const explorerSettings = isObjectQuery ? objectsExplorerSettings : pathsExplorerSettings;
	const settings = explorerSettings.useSettingsSnapshot();

	const take = 10;

	const objectFilter: ObjectFilterArgs = {
		category,
		...(settings.layoutMode === 'media' && {
			kind: [ObjectKindEnum.Image, ObjectKindEnum.Video]
		})
	};

	const objectsCount = useLibraryQuery(['search.objectsCount', { filter: objectFilter }]);

	const objectsQuery = useInfiniteQuery({
		enabled: isObjectQuery,
		queryKey: [
			'search.objects',
			{
				library_id: library.uuid,
				arg: { take, filter: objectFilter }
			}
		] as const,
		queryFn: ({ pageParam: cursor, queryKey }) =>
			rspc.client.query([
				'search.objects',
				{
					...queryKey[1].arg,
					pagination: cursor ? { cursor: { pub_id: cursor } } : undefined
				}
			]),
		getNextPageParam: (lastPage) => lastPage.cursor ?? undefined
	});

	const objectsItems = useMemo(
		() => objectsQuery.data?.pages?.flatMap((d) => d.items),
		[objectsQuery.data]
	);

	const pathsFilter: FilePathFilterArgs = { object: objectFilter };
	const pathsArgs: FilePathSearchArgs = { take, filter: pathsFilter };

	const pathsCount = useLibraryQuery(['search.pathsCount', { filter: pathsFilter }]);

	// TODO: Make a custom double click handler for directories to take users to the location explorer.
	// For now it's not needed because folders shouldn't show.
	const pathsQuery = usePathsInfiniteQuery({
		enabled: !isObjectQuery,
		library,
		arg: pathsArgs,
		settings: pathsExplorerSettings
	});

	const pathsItems = useMemo(
		() => pathsQuery.data?.pages?.flatMap((d) => d.items),
		[pathsQuery.data]
	);

	const loadMore = () => {
		const query = isObjectQuery ? objectsQuery : pathsQuery;
		if (query.hasNextPage && !query.isFetchingNextPage) query.fetchNextPage();
	};

	const shared = {
		loadMore,
		scrollRef: page.ref
	};

	return isObjectQuery
		? // eslint-disable-next-line
		  useExplorer({
				items: objectsItems ?? null,
				count: objectsCount.data,
				settings: objectsExplorerSettings,
				...shared
		  })
		: // eslint-disable-next-line
		  useExplorer({
				items: pathsItems ?? null,
				count: pathsCount.data,
				settings: pathsExplorerSettings,
				...shared
		  });
}
