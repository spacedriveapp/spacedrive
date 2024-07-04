import { Grid, useGrid } from '@virtual-grid/react';
import { useCallback } from 'react';
import { useExplorerLayoutStore } from '@sd/client';

import { useExplorerContext } from '../../Context';
import { getItemData, getItemId, uniqueId } from '../../util';
import { useExplorerViewContext } from '../Context';
import { DragSelect } from '../Grid/DragSelect';
import { GridItem } from '../Grid/Item';
import { useKeySelection } from '../Grid/useKeySelection';
import { GridViewItem } from './Item';

const PADDING = 12;

export const GridView = () => {
	const explorer = useExplorerContext();
	const explorerView = useExplorerViewContext();
	const explorerSettings = explorer.useSettingsSnapshot();
	const layoutStore = useExplorerLayoutStore();

	const itemDetailsHeight =
		(layoutStore.showTags ? 60 : 44) + (explorerSettings.showBytesInGridView ? 20 : 0);
	const itemHeight = explorerSettings.gridItemSize + itemDetailsHeight;

	const BOTTOM_PADDING = layoutStore.showTags ? 16 : 12;
	const grid = useGrid({
		scrollRef: explorer.scrollRef,
		count: explorer.items?.length ?? 0,
		totalCount: explorer.count,
		columns: 'auto',
		size: { width: explorerSettings.gridItemSize, height: itemHeight },
		padding: {
			bottom: BOTTOM_PADDING + (explorerView.scrollPadding?.bottom ?? 0),
			x: PADDING,
			y: PADDING
		},
		gap: explorerSettings.gridGap,
		overscan: explorer.overscan ?? 5,
		onLoadMore: explorer.loadMore,
		getItemId: useCallback(
			(index: number) => getItemId(index, explorer.items ?? []),
			[explorer.items]
		),
		getItemData: useCallback(
			(index: number) => getItemData(index, explorer.items ?? []),
			[explorer.items]
		)
	});

	useKeySelection(grid, { scrollToEnd: true });

	return (
		<DragSelect grid={grid}>
			<Grid grid={grid}>
				{(index) => {
					const item = explorer.items?.[index];
					if (!item) return null;

					return (
						<GridItem
							key={uniqueId(item)}
							index={index}
							item={item}
							style={{ width: grid.itemWidth }}
						>
							{({ selected, cut }) => (
								<GridViewItem data={item} selected={selected} cut={cut} />
							)}
						</GridItem>
					);
				}}
			</Grid>
		</DragSelect>
	);
};
