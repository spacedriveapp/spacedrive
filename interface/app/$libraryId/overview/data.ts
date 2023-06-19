import { iconNames } from '@sd/assets/util';
import { useInfiniteQuery } from '@tanstack/react-query';
import { useMemo } from 'react';
import { Category, useLibraryContext, useRspcLibraryContext } from '@sd/client';
import { getExplorerStore, useExplorerStore } from '~/hooks';

export const IconForCategory: Partial<Record<Category, string>> = {
	Recents: iconNames.Collection,
	Favorites: iconNames.HeartFlat,
	Photos: iconNames.Image,
	Videos: iconNames.Video,
	Movies: iconNames.Movie,
	Music: iconNames.Audio,
	Documents: iconNames.Document,
	Downloads: iconNames.Package,
	Applications: iconNames.Application,
	Games: iconNames.Game,
	Books: iconNames.Book,
	Encrypted: iconNames.EncryptedLock,
	Archives: iconNames.Database,
	Projects: iconNames.Folder,
	Trash: iconNames.Trash
};

const OBJECT_CATEGORIES: Category[] = ['Recents', 'Favorites'];

// this is a gross function so it's in a separate hook :)
export function useItems(category: Category) {
	const explorerStore = useExplorerStore();
	const rspc = useRspcLibraryContext();
	const { library } = useLibraryContext();

	const kind = explorerStore.layoutMode === 'media' ? [5, 7] : undefined;

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
				items: objectsItems,
				query: objectsQuery,
				loadMore
		  }
		: {
				items: pathsItems,
				query: pathsQuery,
				loadMore
		  };
}
