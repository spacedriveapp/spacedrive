import { iconNames } from '@sd/assets/util';
import { useInfiniteQuery } from '@tanstack/react-query';
import { useMemo } from 'react';
import {
	Category,
	ObjectSearchOrdering,
	useLibraryContext,
	useRspcLibraryContext
} from '@sd/client';
import { useExplorerContext } from '../Explorer/Context';
import { getExplorerStore, useExplorerStore } from '../Explorer/store';
import { UseExplorerSettings } from '../Explorer/useExplorer';

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

const OBJECT_CATEGORIES: Category[] = ['Recents', 'Favorites'];

// this is a gross function so it's in a separate hook :)
export function useItems(
	category: Category,
	explorerSettings: UseExplorerSettings<ObjectSearchOrdering>
) {
	const settings = explorerSettings.useSettingsSnapshot();
	const rspc = useRspcLibraryContext();
	const { library } = useLibraryContext();

	const kind = settings.layoutMode === 'media' ? [5, 7] : undefined;

	const isObjectQuery = OBJECT_CATEGORIES.includes(category);

	const objectFilter = { category, kind };

	// TODO: Make a custom double click handler for directories to take users to the location explorer.
	// For now it's not needed because folders shouldn't show.
	const pathsQuery = useInfiniteQuery({
		enabled: !isObjectQuery,
		queryKey: [
			'search.paths',
			{
				library_id: library.uuid,
				arg: {
					take: 50,
					filter: { object: objectFilter }
				}
			}
		] as const,
		queryFn: ({ pageParam: cursor, queryKey }) =>
			rspc.client.query([
				'search.paths',
				{
					...queryKey[1].arg,
					cursor
				}
			]),
		getNextPageParam: (lastPage) => lastPage.cursor ?? undefined,
		onSuccess: () => getExplorerStore().resetNewThumbnails()
	});

	const pathsItems = useMemo(
		() => pathsQuery.data?.pages?.flatMap((d) => d.items),
		[pathsQuery.data]
	);

	const objectsQuery = useInfiniteQuery({
		enabled: isObjectQuery,
		queryKey: [
			'search.objects',
			{
				library_id: library.uuid,
				arg: {
					take: 50,
					filter: objectFilter
				}
			}
		] as const,
		queryFn: ({ pageParam: cursor, queryKey }) =>
			rspc.client.query([
				'search.objects',
				{
					...queryKey[1].arg,
					cursor
				}
			]),
		getNextPageParam: (lastPage) => lastPage.cursor ?? undefined
	});

	const objectsItems = useMemo(
		() => objectsQuery.data?.pages?.flatMap((d) => d.items),
		[objectsQuery.data]
	);

	const loadMore = () => {
		const query = isObjectQuery ? objectsQuery : pathsQuery;
		if (query.hasNextPage && !query.isFetchingNextPage) query.fetchNextPage();
	};

	return isObjectQuery
		? {
				items: objectsItems ?? null,
				query: objectsQuery,
				loadMore
		  }
		: {
				items: pathsItems ?? null,
				query: pathsQuery,
				loadMore
		  };
}
