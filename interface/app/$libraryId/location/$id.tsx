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
import { useEffect } from 'react';
import { useParams, useSearchParams } from 'react-router-dom';
import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { getExplorerStore, useExplorerStore } from '~/hooks/useExplorerStore';
import Explorer from '../Explorer';
import OptionsPanel from '../Explorer/OptionsPanel';
import { KeyManager } from '../KeyManager';
import { TOP_BAR_ICON_STYLE, ToolOption } from '../TopBar';
import TopBarChildren from '../TopBar/TopBarChildren';

export function useExplorerParams() {
	const { id } = useParams<{ id?: string }>();
	const location_id = id ? Number(id) : null;

	const [searchParams] = useSearchParams();
	const path = searchParams.get('path') || '';
	const limit = Number(searchParams.get('limit')) || 100;

	return { location_id, path, limit };
}

export const Component = () => {
	const store = useExplorerStore();
	const toolBarOptions: ToolOption[][] = [
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
				onClick: () => (getExplorerStore().layoutMode = 'media'),
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
				showAtResolution: 'xl:flex',
				topBarActive: store.showInspector
			}
		]
	];

	const { location_id, path, limit } = useExplorerParams();
	// we destructure this since `mutate` is a stable reference but the object it's in is not
	const { mutate: mutateQuickRescan, ...quickRescan } =
		useLibraryMutation('locations.quickRescan');

	const explorerStore = useExplorerStore();

	const explorerState = getExplorerStore();

	useEffect(() => {
		explorerState.locationId = location_id;
		if (location_id !== null) mutateQuickRescan({ location_id, sub_path: path });
	}, [explorerState, location_id, path, mutateQuickRescan]);

	if (location_id === null) throw new Error(`location_id is null!`);

	const explorerData = useLibraryQuery([
		'locations.getExplorerData',
		{
			location_id,
			path: explorerStore.layoutMode === 'media' ? null : path,
			limit,
			cursor: null,
			kind: explorerStore.layoutMode === 'media' ? [5, 7] : null
		}
	]);

	return (
		<>
			<TopBarChildren toolOptions={toolBarOptions} />
			<div className="relative flex w-full flex-col">
				<Explorer data={explorerData.data} />
			</div>
		</>
	);
};
