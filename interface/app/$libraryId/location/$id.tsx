import { useInfiniteQuery } from '@tanstack/react-query';
import { ArrowClockwise, Key, Tag } from 'phosphor-react';
import { useEffect, useMemo } from 'react';
import { useParams, useSearchParams } from 'react-router-dom';
import {
	ExplorerData,
	useLibraryContext,
	useLibraryMutation,
	useRspcLibraryContext
} from '@sd/client';
import { getExplorerStore, useExplorerStore } from '~/hooks/useExplorerStore';
import { useExplorerTopBarOptions } from '~/hooks/useExplorerTopBarOptions';
import Explorer from '../Explorer';
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
	const { location_id, path } = useExplorerParams();
	// we destructure this since `mutate` is a stable reference but the object it's in is not
	const quickRescan = useLibraryMutation('locations.quickRescan');

	const explorerStore = useExplorerStore();
	const explorerState = getExplorerStore();

	useEffect(() => {
		explorerState.locationId = location_id;
		if (location_id !== null) quickRescan.mutate({ location_id, sub_path: path });
	}, [explorerState, location_id, path, quickRescan.mutate]);

	if (location_id === null) throw new Error(`location_id is null!`);

	const ctx = useRspcLibraryContext();
	const { library } = useLibraryContext();

	const query = useInfiniteQuery({
		queryKey: [
			'locations.getExplorerData',
			{
				library_id: library.uuid,
				arg: {
					location_id,
					path: explorerStore.layoutMode === 'media' ? null : path,
					limit: 100,
					kind: explorerStore.layoutMode === 'media' ? [5, 7] : null
				}
			}
		] as const,
		queryFn: async ({ pageParam: cursor, queryKey }): Promise<ExplorerData> => {
			const arg = queryKey[1];
			(arg.arg as any).cursor = cursor;

			return await ctx.client.query(['locations.getExplorerData', arg.arg]);
		},
		getNextPageParam: (lastPage) => lastPage.cursor ?? undefined
	});

	const items = useMemo(() => query.data?.pages.flatMap((d) => d.items), [query.data]);

	return (
		<>
			<TopBarChildren toolOptions={useToolBarOptions()} />
			<div className="relative flex w-full flex-col">
				<Explorer
					items={items}
					onLoadMore={query.fetchNextPage}
					hasNextPage={query.hasNextPage}
				/>
			</div>
		</>
	);
};

const useToolBarOptions = () => {
	const store = useExplorerStore();
	const { explorerViewOptions, explorerControlOptions } = useExplorerTopBarOptions();

	return [
		explorerViewOptions,
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
		explorerControlOptions
	] satisfies ToolOption[][];
};
