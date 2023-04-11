import clsx from 'clsx';
import {
	ArrowClockwise,
	Columns,
	Key,
	MonitorPlay,
	Rows,
	SidebarSimple,
	SlidersHorizontal,
	SquaresFour,
	Tag
} from 'phosphor-react';
import { getExplorerStore, useExplorerStore } from '~/hooks/useExplorerStore';
import { KeyManager } from '../KeyManager';
import OptionsPanel from './OptionsPanel';

export type RoutePaths =
	| 'overview'
	| 'people'
	| 'media'
	| 'spaces'
	| 'debug'
	| 'spacedrop'
	| 'sync'
	| 'location'
	| 'tag'
	| 'settings';

export type groupKeys = 'groupOne' | 'groupTwo' | 'groupThree' | 'groupFour' | 'groupFive';

export interface ToolOptions {
	options: {
		[key in groupKeys]?: {
			icon: JSX.Element;
			onClick?: () => void;
			toolTipLabel: string;
			topBarActive?: boolean;
			popOverComponent?: JSX.Element;
		}[];
	}[];
}

const TOP_BAR_ICON_STYLE = 'm-0.5 w-5 h-5 text-ink-dull';

export const useToolBarRouteOptions = () => {
	const store = useExplorerStore();

	const toolBarRouteOptions: Record<RoutePaths, ToolOptions> = {
		overview: {
			options: [{}]
		},
		location: {
			options: [
				{
					groupOne: [
						{
							toolTipLabel: 'Grid view',
							icon: <SquaresFour className={TOP_BAR_ICON_STYLE} />,
							topBarActive: store.layoutMode === 'grid',
							onClick: () => (getExplorerStore().layoutMode = 'grid')
						},
						{
							toolTipLabel: 'List view',
							icon: <Rows className={TOP_BAR_ICON_STYLE} />,
							topBarActive: store.layoutMode === 'rows',
							onClick: () => (getExplorerStore().layoutMode = 'rows')
						},
						{
							toolTipLabel: 'Columns view',
							icon: <Columns className={TOP_BAR_ICON_STYLE} />,
							topBarActive: store.layoutMode === 'columns',
							onClick: () => (getExplorerStore().layoutMode = 'columns')
						},
						{
							toolTipLabel: 'Media view',
							icon: <MonitorPlay className={TOP_BAR_ICON_STYLE} />,
							topBarActive: store.layoutMode === 'media'
						}
					],
					groupTwo: [
						{
							toolTipLabel: 'Explorer display',
							icon: <SlidersHorizontal className={TOP_BAR_ICON_STYLE} />,
							popOverComponent: <OptionsPanel />
						},
						{
							toolTipLabel: 'Key Manager',
							icon: <Key className={TOP_BAR_ICON_STYLE} />,
							popOverComponent: <KeyManager />
						},
						{
							toolTipLabel: 'Show Inspector',
							onClick: () => (getExplorerStore().showInspector = !store.showInspector),
							icon: (
								<SidebarSimple
									weight={store.showInspector ? 'fill' : 'regular'}
									className={clsx(TOP_BAR_ICON_STYLE, 'scale-x-[-1]')}
								/>
							)
						},
						{
							toolTipLabel: 'Tag Assign Mode',
							icon: (
								<Tag
									weight={store.tagAssignMode ? 'fill' : 'regular'}
									className={TOP_BAR_ICON_STYLE}
								/>
							),
							onClick: () => (getExplorerStore().tagAssignMode = !store.tagAssignMode),
							topBarActive: store.tagAssignMode
						},
						{
							toolTipLabel: 'Regenerate thumbs (temp)',
							icon: <ArrowClockwise className={TOP_BAR_ICON_STYLE} />
						}
					]
				}
			]
		},
		people: {
			options: [{}]
		},
		media: {
			options: [{}]
		},
		spaces: {
			options: [{}]
		},
		debug: {
			options: [{}]
		},
		settings: {
			options: [{}]
		},
		spacedrop: {
			options: [{}]
		},
		tag: {
			options: [{}]
		},
		sync: {
			options: [{}]
		}
	};

	return { toolBarRouteOptions };
};
