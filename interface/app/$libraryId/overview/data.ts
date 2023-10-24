import { iconNames } from '@sd/assets/util';
import { useMemo } from 'react';
import {
	Category,
	FilePathFilterArgs,
	FilePathOrder,
	ObjectFilterArgs,
	ObjectKindEnum,
	ObjectOrder,
	useLibraryContext,
	useLibraryQuery,
	useRspcLibraryContext
} from '@sd/client';

import { useObjectsInfiniteQuery, usePathsInfiniteQuery } from '../Explorer/queries';
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
	Trash: iconNames.Trash,
	Screenshots: iconNames.Screenshot
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
	Trash: 'View all files in your trash',
	Screenshots: 'View all screenshots in your library'
};

export const OBJECT_CATEGORIES: Category[] = ['Recents', 'Favorites'];

// this is a gross function so it's in a separate hook :)
export function useCategoryExplorer(category: Category) {
	const { library } = useLibraryContext();
	const page = usePageLayoutContext();

	const isObjectQuery = OBJECT_CATEGORIES.includes(category);

	const pathsExplorerSettings = useExplorerSettings({
		settings: useMemo(() => createDefaultExplorerSettings<FilePathOrder>(), []),
		orderingKeys: filePathOrderingKeysSchema
	});

	const objectsExplorerSettings = useExplorerSettings({
		settings: useMemo(() => createDefaultExplorerSettings<ObjectOrder>(), []),
		orderingKeys: objectOrderingKeysSchema
	});

	const explorerSettings = isObjectQuery ? objectsExplorerSettings : pathsExplorerSettings;
	const settings = explorerSettings.useSettingsSnapshot();

	const take = 100;

	const objectFilter: ObjectFilterArgs = {
		category,
		...(settings.layoutMode === 'media' && {
			kind: [ObjectKindEnum.Image, ObjectKindEnum.Video]
		})
	};

	const objectsCount = useLibraryQuery(['search.objectsCount', { filter: objectFilter }]);

	const objectsQuery = useObjectsInfiniteQuery({
		enabled: isObjectQuery,
		library,
		arg: { take, filter: objectFilter },
		settings: objectsExplorerSettings
	});

	const objectsItems = useMemo(
		() => objectsQuery.data?.pages?.flatMap((d) => d.items) ?? [],
		[objectsQuery.data]
	);

	const pathsFilter: FilePathFilterArgs = { object: objectFilter };

	const pathsCount = useLibraryQuery(['search.pathsCount', { filter: pathsFilter }]);

	// TODO: Make a custom double click handler for directories to take users to the location explorer.
	// For now it's not needed because folders shouldn't show.
	const pathsQuery = usePathsInfiniteQuery({
		enabled: !isObjectQuery,
		library,
		arg: { take, filter: pathsFilter },
		settings: pathsExplorerSettings
	});

	const pathsItems = useMemo(
		() => pathsQuery.data?.pages?.flatMap((d) => d.items) ?? [],
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
