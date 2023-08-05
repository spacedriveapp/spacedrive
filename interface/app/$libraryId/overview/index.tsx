import { useEffect, useMemo, useState } from 'react';
import 'react-loading-skeleton/dist/skeleton.css';
import { Category } from '@sd/client';
import { ExplorerContext } from '../Explorer/Context';
import ContextMenu, { ObjectItems } from '../Explorer/ContextMenu';
import { Inspector } from '../Explorer/Inspector';
import { DefaultTopBarOptions } from '../Explorer/TopBarOptions';
import View from '../Explorer/View';
import { useExplorerStore } from '../Explorer/store';
import { usePageLayoutContext } from '../PageLayout/Context';
import { TopBarPortal } from '../TopBar/Portal';
import Statistics from '../overview/Statistics';
import { Categories } from './Categories';
import { useItems } from './data';

export const Component = () => {
	const explorerStore = useExplorerStore();
	const { ref: pageRef } = usePageLayoutContext();

	const [selectedCategory, setSelectedCategory] = useState<Category>('Recents');

	const { items, query, loadMore } = useItems(selectedCategory);

	const [selectedItems, setSelectedItems] = useState<Set<number>>(() => new Set());

	const selectedItem = useMemo(
		() => items?.find((item) => item.item.id === [...selectedItems.values()][0]),
		[selectedItems, items]
	);

	useEffect(() => {
		if (pageRef.current) {
			const { scrollTop } = pageRef.current;
			if (scrollTop > 100) pageRef.current.scrollTo({ top: 100 });
		}
	}, [selectedCategory, pageRef]);

	return (
		<ExplorerContext.Provider value={{}}>
			<TopBarPortal right={<DefaultTopBarOptions />} />

			<div>
				<Statistics />

				<Categories selected={selectedCategory} onSelectedChanged={setSelectedCategory} />

				<div className="flex">
					<View
						items={query.isLoading ? null : items || []}
						scrollRef={pageRef}
						onLoadMore={loadMore}
						rowsBeforeLoadMore={5}
						selected={selectedItems}
						onSelectedChange={setSelectedItems}
						top={68}
						className={explorerStore.layoutMode === 'rows' ? 'min-w-0' : undefined}
						contextMenu={
							selectedItem ? (
								<ContextMenu
									item={selectedItem}
									extra={({ object }) => (
										<>
											{object && (
												<ObjectItems.RemoveFromRecents object={object} />
											)}
										</>
									)}
								/>
							) : null
						}
					/>

					{explorerStore.showInspector && (
						<Inspector
							data={selectedItem}
							showThumbnail={explorerStore.layoutMode !== 'media'}
							className="custom-scroll inspector-scroll sticky top-[68px] h-full w-[260px] shrink-0 bg-app pb-4 pl-1.5 pr-1"
						/>
					)}
				</div>
			</div>
		</ExplorerContext.Provider>
	);
};
