import clsx from 'clsx';
import {
	Columns,
	MonitorPlay,
	Rows,
	SidebarSimple,
	SlidersHorizontal,
	SquaresFour
} from 'phosphor-react';
import OptionsPanel from '~/app/$libraryId/Explorer/OptionsPanel';
import { TOP_BAR_ICON_STYLE, ToolOption } from '~/app/$libraryId/TopBar';
import { getExplorerStore, useExplorerStore } from './useExplorerStore';

export const useToolOptions = () => {
	const explorerStore = useExplorerStore();

	const explorerViewOptions: ToolOption[] = [
		{
			toolTipLabel: 'Grid view',
			icon: <SquaresFour className={TOP_BAR_ICON_STYLE} />,
			topBarActive: explorerStore.layoutMode === 'grid',
			onClick: () => (getExplorerStore().layoutMode = 'grid'),
			showAtResolution: 'sm:flex'
		},
		{
			toolTipLabel: 'List view',
			icon: <Rows className={TOP_BAR_ICON_STYLE} />,
			topBarActive: explorerStore.layoutMode === 'rows',
			onClick: () => (getExplorerStore().layoutMode = 'rows'),
			showAtResolution: 'sm:flex'
		},
		{
			toolTipLabel: 'Columns view',
			icon: <Columns className={TOP_BAR_ICON_STYLE} />,
			topBarActive: explorerStore.layoutMode === 'columns',
			onClick: () => (getExplorerStore().layoutMode = 'columns'),
			showAtResolution: 'sm:flex'
		},
		{
			toolTipLabel: 'Media view',
			icon: <MonitorPlay className={TOP_BAR_ICON_STYLE} />,
			topBarActive: explorerStore.layoutMode === 'media',
			onClick: () => (getExplorerStore().layoutMode = 'media'),
			showAtResolution: 'sm:flex'
		}
	];

	const explorerControlOptions: ToolOption[] = [
		{
			toolTipLabel: 'Explorer display',
			icon: <SlidersHorizontal className={TOP_BAR_ICON_STYLE} />,
			popOverComponent: <OptionsPanel />,
			individual: true,
			showAtResolution: 'xl:flex'
		},
		{
			toolTipLabel: 'Show Inspector',
			onClick: () => (getExplorerStore().showInspector = !explorerStore.showInspector),
			icon: (
				<SidebarSimple
					weight={explorerStore.showInspector ? 'fill' : 'regular'}
					className={clsx(TOP_BAR_ICON_STYLE, 'scale-x-[-1]')}
				/>
			),
			individual: true,
			showAtResolution: 'xl:flex',
			topBarActive: explorerStore.showInspector
		}
	];

	return { explorerViewOptions, explorerControlOptions };
};
