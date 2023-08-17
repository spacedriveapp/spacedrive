import { getIcon } from '@sd/assets/util';
import { useEffect, useMemo, useState } from 'react';
import 'react-loading-skeleton/dist/skeleton.css';
import { useSnapshot } from 'valtio';
import { Category, ObjectSearchOrdering } from '@sd/client';
import { useIsDark } from '../../../hooks';
import { ExplorerContextProvider } from '../Explorer/Context';
import ContextMenu, { ObjectItems } from '../Explorer/ContextMenu';
import { Conditional } from '../Explorer/ContextMenu/ConditionalItem';
import { Inspector } from '../Explorer/Inspector';
import { DefaultTopBarOptions } from '../Explorer/TopBarOptions';
import View from '../Explorer/View';
import {
	createDefaultExplorerSettings,
	objectOrderingKeysSchema,
	useExplorerStore
} from '../Explorer/store';
import { useExplorer, useExplorerSettings } from '../Explorer/useExplorer';
import { usePageLayoutContext } from '../PageLayout/Context';
import { TopBarPortal } from '../TopBar/Portal';
import Statistics from '../overview/Statistics';
import { Categories } from './Categories';
import { IconForCategory, IconToDescription, useItems } from './data';

export const Component = () => {
	const explorerStore = useExplorerStore();
	const isDark = useIsDark();
	const page = usePageLayoutContext();

	const explorerSettings = useExplorerSettings({
		settings: useMemo(
			() =>
				createDefaultExplorerSettings<ObjectSearchOrdering>({
					order: null
				}),
			[]
		),
		onSettingsChanged: () => {},
		orderingKeys: objectOrderingKeysSchema
	});

	const [selectedCategory, setSelectedCategory] = useState<Category>('Recents');

	const { items, loadMore } = useItems(selectedCategory, explorerSettings);

	const explorer = useExplorer({
		items,
		loadMore,
		scrollRef: page.ref,
		settings: explorerSettings
	});

	useEffect(() => {
		if (!page.ref.current) return;

		const { scrollTop } = page.ref.current;
		if (scrollTop > 100) page.ref.current.scrollTo({ top: 100 });
	}, [selectedCategory, page.ref]);

	const settings = useSnapshot(explorer.settingsStore);

	return (
		<ExplorerContextProvider explorer={explorer}>
			<TopBarPortal right={<DefaultTopBarOptions />} />

			<Statistics />

			<Categories selected={selectedCategory} onSelectedChanged={setSelectedCategory} />

			<div className="flex flex-1">
				<View
					top={68}
					className={settings.layoutMode === 'list' ? 'min-w-0' : undefined}
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
						showThumbnail={settings.layoutMode !== 'media'}
						className="custom-scroll inspector-scroll sticky top-[68px] h-full w-[260px] shrink-0 bg-app pb-4 pl-1.5 pr-1"
					/>
				)}
			</div>
		</ExplorerContextProvider>
	);
};
