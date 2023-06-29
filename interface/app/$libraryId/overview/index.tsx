import { useMemo, useState } from 'react';
import 'react-loading-skeleton/dist/skeleton.css';
import { Category } from '@sd/client';
import { ExplorerContext } from '../Explorer/Context';
import ContextMenu from '../Explorer/ContextMenu';
// import ContextMenu from '../Explorer/FilePath/ContextMenu';
import { Inspector } from '../Explorer/Inspector';
import { DefaultTopBarOptions } from '../Explorer/TopBarOptions';
import View from '../Explorer/View';
import { useExplorerStore } from '../Explorer/store';
import { usePageLayout } from '../PageLayout';
import { TopBarPortal } from '../TopBar/Portal';
import Statistics from '../overview/Statistics';
import { Categories } from './Categories';
import { useItems } from './data';

export const Component = () => {
	const explorerStore = useExplorerStore();
	const page = usePageLayout();

	const [selectedCategory, setSelectedCategory] = useState<Category>('Recents');

	const { items, query, loadMore } = useItems(selectedCategory);

	const [selectedItemId, setSelectedItemId] = useState<number>();

	const selectedItem = useMemo(
		() => (selectedItemId ? items?.find((item) => item.item.id === selectedItemId) : undefined),
		[selectedItemId, items]
	);

	return (
		<ExplorerContext.Provider value={{}}>
			<TopBarPortal right={<DefaultTopBarOptions />} />

			<div>
				<Statistics />

				<Categories selected={selectedCategory} onSelectedChanged={setSelectedCategory} />

				<div className="flex">
					<View
						items={query.isLoading ? null : items || []}
						// TODO: Fix this type here.
						scrollRef={page?.ref as any}
						onLoadMore={loadMore}
						rowsBeforeLoadMore={5}
						selected={selectedItemId}
						onSelectedChange={setSelectedItemId}
						top={68}
						className={explorerStore.layoutMode === 'rows' ? 'min-w-0' : undefined}
						contextMenu={<ContextMenu item={selectedItem} />}
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
