import * as icons from '@sd/assets/icons';
import { getIcon, iconNames } from '@sd/assets/icons/util';
import { useInfiniteQuery } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import 'react-loading-skeleton/dist/skeleton.css';
import {
	ExplorerItem,
	ObjectKind,
	ObjectKindKey,
	useLibraryContext,
	useLibraryQuery,
	useRspcLibraryContext
} from '@sd/client';
import { z } from '@sd/ui/src/forms';
import { useExplorerStore, useExplorerTopBarOptions, useIsDark } from '~/hooks';
import Explorer from '../Explorer';
import { SEARCH_PARAMS, useExplorerOrder } from '../Explorer/util';
import { usePageLayout } from '../PageLayout';
import { TopBarPortal } from '../TopBar/Portal';
import TopBarOptions from '../TopBar/TopBarOptions';
import CategoryButton from '../overview/CategoryButton';
import Statistics from '../overview/Statistics';

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
	const explorerStore = useExplorerStore();
	const ctx = useRspcLibraryContext();
	const { library } = useLibraryContext();
	const { explorerViewOptions, explorerControlOptions, explorerToolOptions } =
		useExplorerTopBarOptions();

	const [selectedCategory, setSelectedCategory] = useState<string>('Recents');

	// TODO: integrate this into search query
	const recentFiles = useLibraryQuery([
		'search.paths',
		{
			order: { object: { dateAccessed: false } },
			take: 50
		}
	]);
	// this should be redundant once above todo is complete
	const canSearch = !!SearchableCategories[selectedCategory] || selectedCategory === 'Favorites';

	const kind = ObjectKind[SearchableCategories[selectedCategory] || 0] as number;

	const categories = useLibraryQuery(['categories.list']);

	const isFavoritesCategory = selectedCategory === 'Favorites';

	// TODO: Make a custom double click handler for directories to take users to the location explorer.
	// For now it's not needed because folders shouldn't show.
	const query = useInfiniteQuery({
		enabled: canSearch,
		queryKey: [
			'search.paths',
			{
				library_id: library.uuid,
				arg: {
					order: useExplorerOrder(),
					favorite: isFavoritesCategory ? true : undefined,
					...(explorerStore.layoutMode === 'media'
						? {
							kind: [5, 7].includes(kind)
								? [kind]
								: isFavoritesCategory
									? [5, 7]
									: [5, 7, kind]
						}
						: { kind: isFavoritesCategory ? [] : [kind] })
				}
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

	const searchItems = useMemo(() => query.data?.pages?.flatMap((d) => d.items), [query.data]);

	let items: ExplorerItem[] = [];
	switch (selectedCategory) {
		case 'Recents':
			items = recentFiles.data?.items || [];
			break;
		default:
			if (canSearch) {
				items = searchItems || [];
			}
	}

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
