import { getIcon, iconNames } from '@sd/assets/icons/util';
import { useInfiniteQuery } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import 'react-loading-skeleton/dist/skeleton.css';
import {
	FilePathSearchArgs,
	ObjectKind,
	ObjectKindKey,
	useLibraryContext,
	useLibraryQuery,
	useRspcLibraryContext
} from '@sd/client';
import { z } from '@sd/ui/src/forms';
import { useExplorerStore, useExplorerTopBarOptions, useIsDark } from '~/hooks';
import Explorer from '../Explorer';
import { SEARCH_PARAMS } from '../Explorer/util';
import { usePageLayout } from '../PageLayout';
import { TopBarPortal } from '../TopBar/Portal';
import TopBarOptions from '../TopBar/TopBarOptions';
import CategoryButton from '../overview/CategoryButton';
import Statistics from '../overview/Statistics';
import deepMerge from "ts-deepmerge"

// TODO: Replace left hand type with Category enum type (doesn't exist yet)
const CategoryToIcon: Record<string, string> = {
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
};

export type SearchArgs = z.infer<typeof SEARCH_PARAMS>;

export const Component = () => {
	const page = usePageLayout();
	const isDark = useIsDark();
	const { explorerViewOptions, explorerControlOptions, explorerToolOptions } =
		useExplorerTopBarOptions();

	const [selectedCategory, setSelectedCategory] = useState<string>('Recents');

	const { items, query } = useItems(selectedCategory);

	const categories = useLibraryQuery(['categories.list']);

	return (
		<>
			<TopBarPortal
				right={
					<TopBarOptions
						options={[explorerViewOptions, explorerToolOptions, explorerControlOptions]}
					/>
				}
			/>
			<Explorer
				inspectorClassName="!pt-0 !fixed !top-[50px] !right-[10px]  !w-[260px]"
				explorerClassName="!overflow-visible" // required to ensure categories are sticky, remove with caution
				viewClassName="!pl-0 !pt-0 !h-auto"
				items={items}
				onLoadMore={query.fetchNextPage}
				hasNextPage={query.hasNextPage}
				isFetchingNextPage={query.isFetchingNextPage}
				scrollRef={page?.ref}
			>
				<Statistics />
				<div className="no-scrollbar sticky top-0 z-10 mt-2 flex space-x-[1px] overflow-x-scroll bg-app/90 px-5 py-1.5 backdrop-blur">
					{categories.data?.map((category) => {
						const iconString = CategoryToIcon[category.name] || 'Document';
						return (
							<CategoryButton
								key={category.name}
								category={category.name}
								icon={getIcon(iconString, isDark)}
								items={category.count}
								selected={selectedCategory === category.name}
								onClick={() => setSelectedCategory(category.name)}
							/>
						);
					})}
				</div>
			</Explorer>
		</>
	);
};

function getCategorySearchArgs(category: string): FilePathSearchArgs {
	if (category === 'Recents')
		return {
			order: { object: { dateAccessed: false } }
		};

	if (category === 'Favourites')
		return {
			filter: {
				object: {
					favorite: true
				}
			}
		};

	return {};
}

// this is a gross function so it's in a separate hook :)
function useItems(selectedCategory: string) {
	const explorerStore = useExplorerStore();
	const ctx = useRspcLibraryContext();
	const { library } = useLibraryContext();

	const searchableCategory = SearchableCategories[selectedCategory];
	const searchableCategoryKind =
		searchableCategory !== undefined ? (ObjectKind[searchableCategory] as number) : undefined;

	const kind = searchableCategoryKind ? [searchableCategoryKind] : undefined;
	if (explorerStore.layoutMode === 'media') [5, 7].forEach((v) => kind?.push(v));

	// TODO: Make a custom double click handler for directories to take users to the location explorer.
	// For now it's not needed because folders shouldn't show.
	const query = useInfiniteQuery({
		queryKey: [
			'search.paths',
			{
				library_id: library.uuid,
				arg: deepMerge(
					{
						take: 50,
						filter: {
							object: { kind }
						},
					},
					getCategorySearchArgs(selectedCategory)
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
		getNextPageParam: (lastPage) => lastPage.cursor ?? undefined
	});

	const items = useMemo(() => query.data?.pages?.flatMap((d) => d.items), [query.data]);

	return { query, items }
}
