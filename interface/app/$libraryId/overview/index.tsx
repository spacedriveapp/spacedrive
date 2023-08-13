import { getIcon } from '@sd/assets/util';
import { useEffect, useState } from 'react';
import 'react-loading-skeleton/dist/skeleton.css';
import { Category } from '@sd/client';
import { useIsDark } from '../../../hooks';
import { ExplorerContext } from '../Explorer/Context';
import ContextMenu, { ObjectItems } from '../Explorer/ContextMenu';
import { Conditional } from '../Explorer/ContextMenu/ConditionalItem';
import { Inspector } from '../Explorer/Inspector';
import { DefaultTopBarOptions } from '../Explorer/TopBarOptions';
import View from '../Explorer/View';
import { useExplorerStore } from '../Explorer/store';
import { useExplorer } from '../Explorer/useExplorer';
import { usePageLayoutContext } from '../PageLayout/Context';
import { TopBarPortal } from '../TopBar/Portal';
import Statistics from '../overview/Statistics';
import { Categories, CategoryList } from './Categories';
import { IconForCategory, useItems } from './data';

const IconToDescription = {
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

export const Component = () => {
	const explorerStore = useExplorerStore();
	const isDark = useIsDark();
	const page = usePageLayoutContext();

	const [selectedCategory, setSelectedCategory] = useState<Category>('Recents');

	const { items, loadMore } = useItems(selectedCategory);

	const explorer = useExplorer({
		items,
		loadMore,
		scrollRef: page.ref
	});

	useEffect(() => {
		if (!page.ref.current) return;

		const { scrollTop } = page.ref.current;
		if (scrollTop > 100) page.ref.current.scrollTo({ top: 100 });
	}, [selectedCategory, page.ref]);

	return (
		<ExplorerContext.Provider value={explorer}>
			<TopBarPortal right={<DefaultTopBarOptions />} />

			<Statistics />

			<Categories selected={selectedCategory} onSelectedChanged={setSelectedCategory} />

			<div className="flex flex-1">
				<View
					top={68}
					className={explorerStore.layoutMode === 'rows' ? 'min-w-0' : undefined}
					contextMenu={
						<ContextMenu>
							{() => <Conditional items={[ObjectItems.RemoveFromRecents]} />}
						</ContextMenu>
					}
					emptyNotice={
						<div className="flex h-full flex-col items-center justify-center text-white">
							<img
								src={getIcon(
									IconForCategory[selectedCategory] || 'Document',
									isDark
								)}
								className="h-32 w-32"
							/>
							<h1 className="mt-4 text-lg font-bold">{selectedCategory}</h1>
							<p className="mt-1 text-sm text-ink-dull">
								{IconToDescription[selectedCategory]}
							</p>
						</div>
					}
				/>

				{explorerStore.showInspector && (
					<Inspector
						showThumbnail={explorerStore.layoutMode !== 'media'}
						className="custom-scroll inspector-scroll sticky top-[68px] h-full w-[260px] shrink-0 bg-app pb-4 pl-1.5 pr-1"
					/>
				)}
			</div>
		</ExplorerContext.Provider>
	);
};
