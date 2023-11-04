import { getIcon } from '@sd/assets/util';
import { useEffect, useState } from 'react';

import 'react-loading-skeleton/dist/skeleton.css';

import { useSnapshot } from 'valtio';
import { Category } from '@sd/client';

import { useIsDark } from '../../../hooks';
import { ExplorerContextProvider } from '../Explorer/Context';
import ContextMenu, { ObjectItems } from '../Explorer/ContextMenu';
import { Conditional } from '../Explorer/ContextMenu/ConditionalItem';
import { DefaultTopBarOptions } from '../Explorer/TopBarOptions';
import View from '../Explorer/View';
import Statistics from '../overview/Statistics';
import { usePageLayoutContext } from '../PageLayout/Context';
import { TopBarPortal } from '../TopBar/Portal';
import { Categories } from './Categories';
import { IconForCategory, IconToDescription, useCategoryExplorer } from './data';
import Inspector from './Inspector';

export const Component = () => {
	const isDark = useIsDark();
	const page = usePageLayoutContext();

	const [selectedCategory, setSelectedCategory] = useState<Category>('Recents');

	const explorer = useCategoryExplorer(selectedCategory);

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
			{/* <div className="mt-2 w-full" /> */}
			<Categories selected={selectedCategory} onSelectedChanged={setSelectedCategory} />

			<div className="flex flex-1">
				<View
					top={114}
					className={settings.layoutMode === 'list' ? 'min-w-0' : undefined}
					contextMenu={
						<ContextMenu>
							<Conditional items={[ObjectItems.RemoveFromRecents]} />
						</ContextMenu>
					}
					emptyNotice={
						<div className="flex flex-1 flex-col items-center justify-center text-white">
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
				<Inspector />
			</div>
		</ExplorerContextProvider>
	);
};
