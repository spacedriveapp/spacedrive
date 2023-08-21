import clsx from 'clsx';
import {
	ArrowClockwise,
	Key,
	MonitorPlay,
	Rows,
	SidebarSimple,
	SlidersHorizontal,
	SquaresFour,
	Tag
} from 'phosphor-react';
import { useEffect, useRef } from 'react';
import { useRspcLibraryContext } from '@sd/client';
import { KeyManager } from '../KeyManager';
import TopBarOptions, { TOP_BAR_ICON_STYLE, ToolOption } from '../TopBar/TopBarOptions';
import { useExplorerContext } from './Context';
import OptionsPanel from './OptionsPanel';
import { getExplorerStore, useExplorerStore } from './store';
import { useExplorerSearchParams } from './util';

export const useExplorerTopBarOptions = () => {
	const explorerStore = useExplorerStore();
	const explorer = useExplorerContext();

	const settings = explorer.useSettingsSnapshot();

	const viewOptions: ToolOption[] = [
		{
			toolTipLabel: 'Grid view',
			icon: <SquaresFour className={TOP_BAR_ICON_STYLE} />,
			topBarActive: settings.layoutMode === 'grid',
			onClick: () => (explorer.settingsStore.layoutMode = 'grid'),
			showAtResolution: 'sm:flex'
		},
		{
			toolTipLabel: 'List view',
			icon: <Rows className={TOP_BAR_ICON_STYLE} />,
			topBarActive: settings.layoutMode === 'list',
			onClick: () => (explorer.settingsStore.layoutMode = 'list'),
			showAtResolution: 'sm:flex'
		},
		// {
		// 	toolTipLabel: 'Columns view',
		// 	icon: <Columns className={TOP_BAR_ICON_STYLE} />,
		// 	topBarActive: explorerStore.layoutMode === 'columns',
		// 	// onClick: () => (getExplorerStore().layoutMode = 'columns'),
		// 	showAtResolution: 'sm:flex'
		// },
		{
			toolTipLabel: 'Media view',
			icon: <MonitorPlay className={TOP_BAR_ICON_STYLE} />,
			topBarActive: settings.layoutMode === 'media',
			onClick: () => (explorer.settingsStore.layoutMode = 'media'),
			showAtResolution: 'sm:flex'
		}
	];

	const controlOptions: ToolOption[] = [
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

	// subscription so that we can cancel it if in progress
	const quickRescanSubscription = useRef<() => void | undefined>();

	// gotta clean up any rescan subscriptions if the exist
	useEffect(() => () => quickRescanSubscription.current?.(), []);

	const { client } = useRspcLibraryContext();

	const { parent } = useExplorerContext();

	const [{ path }] = useExplorerSearchParams();

	const toolOptions = [
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
					weight={explorerStore.tagAssignMode ? 'fill' : 'regular'}
					className={TOP_BAR_ICON_STYLE}
				/>
			),
			onClick: () => (getExplorerStore().tagAssignMode = !explorerStore.tagAssignMode),
			topBarActive: explorerStore.tagAssignMode,
			individual: true,
			showAtResolution: 'xl:flex'
		},
		parent?.type === 'Location' && {
			toolTipLabel: 'Reload',
			onClick: () => {
				quickRescanSubscription.current?.();
				quickRescanSubscription.current = client.addSubscription(
					[
						'locations.quickRescan',
						{
							location_id: parent.location.id,
							sub_path: path ?? ''
						}
					],
					{ onData() {} }
				);
			},
			icon: <ArrowClockwise className={TOP_BAR_ICON_STYLE} />,
			individual: true,
			showAtResolution: 'xl:flex'
		}
	].filter(Boolean) as ToolOption[];

	return {
		viewOptions,
		controlOptions,
		toolOptions
	};
};

export const DefaultTopBarOptions = () => {
	const options = useExplorerTopBarOptions();

	return (
		<TopBarOptions
			options={[options.viewOptions, options.toolOptions, options.controlOptions]}
		/>
	);
};
