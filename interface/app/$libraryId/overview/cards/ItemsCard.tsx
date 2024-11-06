import { UseQueryResult } from '@tanstack/react-query';
import { useNavigate } from 'react-router';
import { ObjectOrder, objectOrderingKeysSchema, useExplorerLayoutStore } from '@sd/client';
import { Button } from '@sd/ui';
import { useLocale } from '~/hooks';

import { OverviewCard } from '..';
import { ExplorerContextProvider } from '../../Explorer/Context';
import { createDefaultExplorerSettings } from '../../Explorer/store';
import { useExplorer, useExplorerSettings } from '../../Explorer/useExplorer';
import { uniqueId } from '../../Explorer/util';
import { ExplorerViewContext } from '../../Explorer/View/Context';
import { SimpleGridItem } from '../../Explorer/View/Grid/SimpleGridItem';
import { GridViewItem } from '../../Explorer/View/GridView/Item';
import HorizontalScroll from '../Layout/HorizontalScroll';

interface ItemsCardProps {
	title: string;
	query: UseQueryResult<{ items: any[] }>;
	buttonText: string;
	buttonLink: string;
	maxItems?: number;
}

export const ItemsCard = ({
	title,
	query,
	buttonText,
	buttonLink,
	maxItems = 20
}: ItemsCardProps) => {
	const navigate = useNavigate();
	const { t } = useLocale();
	const layoutStore = useExplorerLayoutStore();

	const explorerSettings = useExplorerSettings({
		settings: {
			...createDefaultExplorerSettings<ObjectOrder>({
				order: { field: 'dateAccessed', value: 'Desc' }
			}),
			gridItemSize: 80,
			gridGap: 9
		},
		orderingKeys: objectOrderingKeysSchema
	});

	const items = query.data?.items ?? [];
	const displayItems = items.slice(0, maxItems);

	const explorer = useExplorer({
		items: displayItems,
		settings: explorerSettings,
		isFetching: query.isLoading,
		isFetchingNextPage: false
	});

	const itemDetailsHeight =
		(layoutStore.showTags ? 60 : 44) +
		(explorerSettings.settingsStore.showBytesInGridView ? 20 : 0);
	const itemHeight = explorerSettings.settingsStore.gridItemSize + itemDetailsHeight;

	return (
		<>
			<ExplorerContextProvider explorer={explorer}>
				<ExplorerViewContext.Provider
					value={{
						ref: { current: null },
						selectable: true,
						getActiveItemIndex: () => -1,
						getFirstActiveItemIndex: () => -1,
						updateActiveItem: () => {},
						updateFirstActiveItem: () => {},
						handleWindowsGridShiftSelection: () => {}
					}}
				>
					<HorizontalScroll>
						<div
							className="mt-4 flex gap-2"
							style={{
								height: itemHeight
							}}
						>
							{displayItems.map((item) => (
								<div
									key={uniqueId(item)}
									style={{
										width: explorerSettings.settingsStore.gridItemSize,
										height: itemHeight
									}}
								>
									<SimpleGridItem item={item}>
										{({ selected, cut }) => (
											<GridViewItem
												data={item}
												selected={selected}
												cut={cut}
											/>
										)}
									</SimpleGridItem>
								</div>
							))}
						</div>
					</HorizontalScroll>
				</ExplorerViewContext.Provider>
			</ExplorerContextProvider>
			{/* <Button
				variant="subtle"
				size="sm"
				onClick={() => navigate(buttonLink)}
				className="mt-2 w-full"
			>
				{t(buttonText)}
			</Button> */}
		</>
	);
};
