import { useEffect, useMemo, useState } from 'react';
import 'react-loading-skeleton/dist/skeleton.css';
import { Category } from '@sd/client';
import { ExplorerContext } from '../Explorer/Context';
import ContextMenu from '../Explorer/ContextMenu';
// import ContextMenu from '../Explorer/FilePath/ContextMenu';
import { Inspector } from '../Explorer/Inspector';
import { DefaultTopBarOptions } from '../Explorer/TopBarOptions';
import View from '../Explorer/View';
import { useExplorerStore } from '../Explorer/store';
import { uniqueId } from '../Explorer/util';
import { usePageLayoutContext } from '../PageLayout/Context';
import { TopBarPortal } from '../TopBar/Portal';
import Statistics from '../overview/Statistics';
import { Categories } from './Categories';
import { useItems } from './data';

export const Component = () => {
	const explorerStore = useExplorerStore();
	const page = usePageLayoutContext();

	const [selectedCategory, setSelectedCategory] = useState<Category>('Recents');

	const { items, query, loadMore } = useItems(selectedCategory);

	const [selectedItemId, setSelectedItemId] = useState<string>();

	const selectedItem = useMemo(
		() =>
			selectedItemId ? items?.find((item) => uniqueId(item) === selectedItemId) : undefined,
		[selectedItemId, items]
	);

	useEffect(() => {
		if (page?.ref.current) {
			const { scrollTop } = page.ref.current;
			if (scrollTop > 100) page.ref.current.scrollTo({ top: 100 });
		}
	}, [selectedCategory, page?.ref]);

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
						contextMenu={selectedItem ? <ContextMenu item={selectedItem} /> : null}
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
