import { useMemo, useState } from 'react';
import 'react-loading-skeleton/dist/skeleton.css';
import { useKey } from 'rooks';
import { Category } from '@sd/client';
import { z } from '@sd/ui/src/forms';
import { getExplorerStore, useExplorerStore, useExplorerTopBarOptions } from '~/hooks';
import ContextMenu from '../Explorer/File/ContextMenu';
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

	const { items, query, loadMore } = useItems(selectedCategory);

	const [selectedItemId, setSelectedItemId] = useState<number>();

	const selectedItem = useMemo(
		() => (selectedItemId ? items?.find((item) => item.item.id === selectedItemId) : undefined),
		[selectedItemId]
	);

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
						// TODO: Fix this type here.
						scrollRef={page?.ref as any}
						onLoadMore={loadMore}
						rowsBeforeLoadMore={5}
						selected={selectedItemId}
						onSelectedChange={setSelectedItemId}
						top={68}
						className={explorerStore.layoutMode === 'rows' ? 'min-w-0' : undefined}
						contextMenu={selectedItem && <ContextMenu data={selectedItem} />}
						emptyNotice={null}
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
		</>
	);
};
