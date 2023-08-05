import { useEffect, useState } from 'react';
import 'react-loading-skeleton/dist/skeleton.css';
import { Category } from '@sd/client';
import { ExplorerContext } from '../Explorer/Context';
import { Inspector } from '../Explorer/Inspector';
import { DefaultTopBarOptions } from '../Explorer/TopBarOptions';
import View from '../Explorer/View';
import { useExplorerStore } from '../Explorer/store';
import { useExplorer } from '../Explorer/useExplorer';
import { usePageLayoutContext } from '../PageLayout/Context';
import { TopBarPortal } from '../TopBar/Portal';
import Statistics from '../overview/Statistics';
import { Categories } from './Categories';
import { useItems } from './data';

export const Component = () => {
	const explorerStore = useExplorerStore();
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

			<div>
				<Statistics />

				<Categories selected={selectedCategory} onSelectedChanged={setSelectedCategory} />

				<div className="flex">
					<View
						top={68}
						className={explorerStore.layoutMode === 'rows' ? 'min-w-0' : undefined}
					/>

					{explorerStore.showInspector && (
						<Inspector
							showThumbnail={explorerStore.layoutMode !== 'media'}
							className="custom-scroll inspector-scroll sticky top-[68px] h-full w-[260px] shrink-0 bg-app pb-4 pl-1.5 pr-1"
						/>
					)}
				</div>
			</div>
		</ExplorerContext.Provider>
	);
};

// contextMenu={
// 	selectedItem ? (
// 		<ContextMenu
// 			item={selectedItem}
// 			extra={({ object }) => (
// 				<>
// 					{object && (
// 						<ObjectItems.RemoveFromRecents object={object} />
// 					)}
// 					</>
// 			)}
// 			/>
// 	) : null
// }
