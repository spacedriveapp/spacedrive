import { iconNames } from '@sd/assets/util';
import { useInfiniteQuery } from '@tanstack/react-query';
import { useMemo } from 'react';
import deepMerge from 'ts-deepmerge';
import {
	Category,
	FilePathSearchArgs,
	ObjectKind,
	ObjectKindKey,
	ObjectSearchArgs,
	useLibraryContext,
	useRspcLibraryContext
} from '@sd/client';
import { getExplorerStore, useExplorerStore } from '~/hooks';
import { useExplorerOrder } from '../Explorer/util';

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

// Map the category to the ObjectKind for searching
const SearchableCategories: Record<string, ObjectKindKey> = {
	Photos: 'Image',
	Videos: 'Video',
	Music: 'Audio',
	Documents: 'Document',
	Encrypted: 'Encrypted',
	Books: 'Book'
} satisfies Partial<Record<Category, ObjectKindKey>>;

const OBJECT_CATEGORIES: Category[] = ['Recents', 'Favorites'];

// this is a gross function so it's in a separate hook :)
export function useItems(selectedCategory: Category) {
	const explorerStore = useExplorerStore();
	const ctx = useRspcLibraryContext();
	const { library } = useLibraryContext();

	const searchableCategory = SearchableCategories[selectedCategory];
	const searchableCategoryKind =
		searchableCategory !== undefined ? (ObjectKind[searchableCategory] as number) : undefined;

	const kind = searchableCategoryKind ? [searchableCategoryKind] : undefined;
	if (explorerStore.layoutMode === 'media') [5, 7].forEach((v) => kind?.push(v));

	const isObjectQuery = OBJECT_CATEGORIES.includes(selectedCategory);

	// TODO: Make a custom double click handler for directories to take users to the location explorer.
	// For now it's not needed because folders shouldn't show.
	const pathsQuery = useInfiniteQuery({
		enabled: !isObjectQuery,
		queryKey: [
			'search.paths',
			{
				library_id: library.uuid,
				arg: deepMerge(
					{
						take: 50,
						filter: {
							object: { kind }
						}
					},
					categorySearchPathsArgs(selectedCategory)
				)
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
				arg: deepMerge(
					{
						take: 50,
						filter: {
							kind
						}
					},
					categorySearchObjectsArgs(selectedCategory)
				)
			}
		] as const,
		queryFn: ({ pageParam: cursor, queryKey }) =>
			ctx.client.query([
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

function categorySearchPathsArgs(_: string): FilePathSearchArgs {
	return {};
}

function categorySearchObjectsArgs(category: string): ObjectSearchArgs {
	if (category === 'Recents')
		return {
			order: { dateAccessed: 'Desc' },
			filter: {
				dateAccessed: {
					not: null
				}
			}
		};

	if (category === 'Favorites')
		return {
			filter: {
				favorite: true
			}
		};

	return {};
}
