import { useMemo, useState } from 'react';
import 'react-loading-skeleton/dist/skeleton.css';
import { Category } from '@sd/client';
import { z } from '@sd/ui/src/forms';
import { useExplorerStore, useExplorerTopBarOptions } from '~/hooks';
import { Inspector } from '../Explorer/Inspector';
import View from '../Explorer/View';
import { SEARCH_PARAMS } from '../Explorer/util';
import { usePageLayout } from '../PageLayout';
import { TopBarPortal } from '../TopBar/Portal';
import TopBarOptions from '../TopBar/TopBarOptions';
import Statistics from '../overview/Statistics';
import { Categories } from './Categories';
import { useItems } from './data';

export type SearchArgs = z.infer<typeof SEARCH_PARAMS>;

export const Component = () => {
	const explorerStore = useExplorerStore();
	const page = usePageLayout();
	const { explorerViewOptions, explorerControlOptions, explorerToolOptions } =
		useExplorerTopBarOptions();

	const [selectedCategory, setSelectedCategory] = useState<Category>('Recents');

	const { items, query } = useItems(selectedCategory);

	const [selectedItems, setSelectedItems] = useState<number[]>([]);

	// TODO: Instead of filter fetch item in inspector?
	const selectedItem = useMemo(
		() => items?.filter((item) => item.item.id === selectedItems[0])[0],
		[selectedItems[0]]
	);

	const loadMore = () => {
		if (query.hasNextPage && !query.isFetchingNextPage) query.fetchNextPage();
	};

	return (
		<>
			<TopBarPortal
				right={
					<TopBarOptions
						options={[explorerViewOptions, explorerToolOptions, explorerControlOptions]}
					/>
				}
			/>

			<div>
				<Statistics />

				<Categories selected={selectedCategory} onSelectedChanged={setSelectedCategory} />

				<div className="flex">
					<View
						layout={explorerStore.layoutMode}
						items={query.isLoading ? null : items || []}
						scrollRef={page?.ref!}
						onLoadMore={loadMore}
						rowsBeforeLoadMore={5}
						selected={selectedItems}
						onSelectedChange={setSelectedItems}
						top={68}
					/>
					{explorerStore.showInspector && (
						<Inspector
							data={selectedItem}
							showThumbnail={explorerStore.layoutMode !== 'media'}
							className="custom-scroll inspector-scroll sticky top-[68px] h-full w-[260px] flex-shrink-0 bg-app pb-4 pl-1.5 pr-1"
						/>
					)}
				</div>
			</div>
		</>
	);
};
