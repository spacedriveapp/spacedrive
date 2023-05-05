import { useInfiniteQuery } from '@tanstack/react-query';
import { ArrowClockwise, Key, Tag } from 'phosphor-react';
import { useEffect, useMemo } from 'react';
import { useKey } from 'rooks';
import { z } from 'zod';
import {
	ExplorerData,
	useLibraryContext,
	useLibraryMutation,
	useRspcLibraryContext
} from '@sd/client';
import { dialogManager } from '@sd/ui';
import { useZodRouteParams } from '~/hooks';
import { getExplorerStore, useExplorerStore } from '~/hooks/useExplorerStore';
import { useExplorerTopBarOptions } from '~/hooks/useExplorerTopBarOptions';
import Explorer from '../Explorer';
import DeleteDialog from '../Explorer/File/DeleteDialog';
import { useExplorerSearchParams } from '../Explorer/util';
import { KeyManager } from '../KeyManager';
import { TOP_BAR_ICON_STYLE, ToolOption } from '../TopBar';
import TopBarChildren from '../TopBar/TopBarChildren';

const PARAMS = z.object({
	id: z.coerce.number()
});

export const Component = () => {
	const [{ path }] = useExplorerSearchParams();
	const { id: location_id } = useZodRouteParams(PARAMS);

	// we destructure this since `mutate` is a stable reference but the object it's in is not
	const { mutate: quickRescan } = useLibraryMutation('locations.quickRescan');

	const explorerStore = getExplorerStore();

	useEffect(() => {
		explorerStore.locationId = location_id;
		if (location_id !== null) quickRescan({ location_id, sub_path: path ?? '' });
	}, [explorerStore, location_id, path, quickRescan]);

	const { query, items } = useItems();

	useKey('Delete', (e) => {
		e.preventDefault();

		const explorerStore = getExplorerStore();

		if (explorerStore.selectedRowIndex === null) return;

		const file = items?.[explorerStore.selectedRowIndex];

		if (!file) return;

		dialogManager.create((dp) => (
			<DeleteDialog {...dp} location_id={location_id} path_id={file.item.id} />
		));
	});

	return (
		<>
			<TopBarChildren toolOptions={useToolBarOptions()} />
			<div className="relative flex w-full flex-col">
				<Explorer
					items={items}
					onLoadMore={query.fetchNextPage}
					hasNextPage={query.hasNextPage}
					isFetchingNextPage={query.isFetchingNextPage}
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

const useItems = () => {
	const { id: location_id } = useZodRouteParams(PARAMS);
	const [{ path, limit }] = useExplorerSearchParams();

	const ctx = useRspcLibraryContext();
	const { library } = useLibraryContext();

	const explorerState = useExplorerStore();

	const query = useInfiniteQuery({
		queryKey: [
			'locations.getExplorerData',
			{
				library_id: library.uuid,
				arg: {
					location_id,
					path: explorerState.layoutMode === 'media' ? null : path,
					limit,
					kind: explorerState.layoutMode === 'media' ? [5, 7] : null
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

	return { query, items };
};
