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
import OptionsPanel from '../Explorer/OptionsPanel';
import { KeyManager } from '../KeyManager';

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

// string concatination is incorrect usage of Tailwind - the complete and correct way is to use the full string
// this is why I added :flex to the end of each string
//sm is  640px
//md is 768px
//lg is 1024px
//xl is 1280px
//2xl is 1536px
export type ShowAtResolution = 'sm:flex' | 'md:flex' | 'lg:flex' | 'xl:flex' | '2xl:flex';

export interface ToolOption {
	icon: JSX.Element;
	onClick?: () => void;
	individual?: boolean;
	toolTipLabel: string;
	topBarActive?: boolean;
	popOverComponent?: JSX.Element;
	showAtResolution: ShowAtResolution;
}
export interface ToolOptions {
	options: {
		[key: string]: ToolOption[];
	}[];
}

export const TOP_BAR_ICON_STYLE = 'm-0.5 w-5 h-5 text-ink-dull';

export const useToolBarRouteOptions = () => {
	const store = useExplorerStore();

	const toolBarRouteOptions: Record<RoutePaths, { options: ToolOption[][] }> = {
		overview: {
			options: [[]]
		},
		location: {
			options: [
				[
					{
						toolTipLabel: 'Grid view',
						icon: <SquaresFour className={TOP_BAR_ICON_STYLE} />,
						topBarActive: store.layoutMode === 'grid',
						onClick: () => (getExplorerStore().layoutMode = 'grid'),
						showAtResolution: 'sm:flex'
					},
					{
						toolTipLabel: 'List view',
						icon: <Rows className={TOP_BAR_ICON_STYLE} />,
						topBarActive: store.layoutMode === 'rows',
						onClick: () => (getExplorerStore().layoutMode = 'rows'),
						showAtResolution: 'sm:flex'
					},
					{
						toolTipLabel: 'Columns view',
						icon: <Columns className={TOP_BAR_ICON_STYLE} />,
						topBarActive: store.layoutMode === 'columns',
						onClick: () => (getExplorerStore().layoutMode = 'columns'),
						showAtResolution: 'sm:flex'
					},
					{
						toolTipLabel: 'Media view',
						icon: <MonitorPlay className={TOP_BAR_ICON_STYLE} />,
						topBarActive: store.layoutMode === 'media',
						showAtResolution: 'sm:flex'
					}
				],
				[
					{
						toolTipLabel: 'Key Manager',
						icon: <Key className={TOP_BAR_ICON_STYLE} />,
						popOverComponent: <KeyManager />,
						individual: true,
						showAtResolution: 'xl:flex'
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
						topBarActive: store.tagAssignMode,
						individual: true,
						showAtResolution: 'xl:flex'
					},
					{
						toolTipLabel: 'Regenerate thumbs (temp)',
						icon: <ArrowClockwise className={TOP_BAR_ICON_STYLE} />,
						individual: true,
						showAtResolution: 'xl:flex'
					}
				],
				[
					{
						toolTipLabel: 'Explorer display',
						icon: <SlidersHorizontal className={TOP_BAR_ICON_STYLE} />,
						popOverComponent: <OptionsPanel />,
						individual: true,
						showAtResolution: 'xl:flex'
					},
					{
						toolTipLabel: 'Show Inspector',
						onClick: () => (getExplorerStore().showInspector = !store.showInspector),
						icon: (
							<SidebarSimple
								weight={store.showInspector ? 'fill' : 'regular'}
								className={clsx(TOP_BAR_ICON_STYLE, 'scale-x-[-1]')}
							/>
						),
						individual: true,
						showAtResolution: 'xl:flex'
					}
				]
			]
		},
		people: {
			options: [[]]
		},
		media: {
			options: [[]]
		},
		spaces: {
			options: [[]]
		},
		debug: {
			options: [[]]
		},
		settings: {
			options: [[]]
		},
		spacedrop: {
			options: [[]]
		},
		tag: {
			options: [[]]
		},
		sync: {
			options: [[]]
		}
	};

	return { toolBarRouteOptions };
};
