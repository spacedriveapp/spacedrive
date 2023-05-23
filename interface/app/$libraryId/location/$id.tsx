import { useInfiniteQuery } from '@tanstack/react-query';
import { useEffect, useMemo } from 'react';
import { z } from 'zod';
import {
	ExplorerItem,
	useLibraryContext,
	useLibraryMutation,
	useLibraryQuery,
	useRspcLibraryContext
} from '@sd/client';
import { Folder } from '@sd/ui';
import {
	getExplorerStore,
	useExplorerStore,
	useExplorerTopBarOptions,
	useZodRouteParams
} from '~/hooks';
import Explorer from '../Explorer';
import { useExplorerOrder, useExplorerSearchParams } from '../Explorer/util';
import { TopBarPortal } from '../TopBar/Portal';
import TopBarOptions from '../TopBar/TopBarOptions';
import useKeyDeleteFile from '~/hooks/useKeyDeleteFile';

const PARAMS = z.object({
	id: z.coerce.number()
});

export const Component = () => {
	const [{ path }] = useExplorerSearchParams();
	const { id: location_id } = useZodRouteParams(PARAMS);
	const { explorerViewOptions, explorerControlOptions, explorerToolOptions } =
		useExplorerTopBarOptions();

	const location = useLibraryQuery(['locations.get', location_id]);

	// we destructure this since `mutate` is a stable reference but the object it's in is not
	const { mutate: quickRescan } = useLibraryMutation('locations.quickRescan');

	const explorerStore = getExplorerStore();
	useEffect(() => {
		explorerStore.locationId = location_id;
		if (location_id !== null) quickRescan({ location_id, sub_path: path ?? '' });
	}, [explorerStore, location_id, path, quickRescan]);

	const { query, items } = useItems();
	const file = explorerStore.selectedRowIndex !== null && items?.[explorerStore.selectedRowIndex];
	useKeyDeleteFile(file as ExplorerItem, location_id)


	return (
		<>
			<TopBarPortal
				left={
					<>
						<Folder size={22} className="-mt-[1px] ml-3 mr-2 inline-block" />
						<span className="text-sm font-medium">
							{path ? getLastSectionOfPath(path) : location.data?.name}
						</span>
					</>
				}
				right={
					<TopBarOptions
						options={[explorerViewOptions, explorerToolOptions, explorerControlOptions]}
					/>
				}
			/>
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

const useItems = () => {
	const { id: locationId } = useZodRouteParams(PARAMS);
	const [{ path, take }] = useExplorerSearchParams();

	const ctx = useRspcLibraryContext();
	const { library } = useLibraryContext();

	const explorerState = useExplorerStore();

	const query = useInfiniteQuery({
		queryKey: [
			'search.paths',
			{
				library_id: library.uuid,
				arg: {
					order: useExplorerOrder(),
					filter: {
						locationId,
						...(explorerState.layoutMode === 'media'
							? { object: { kind: [5, 7] } }
							: { path: path ?? '' })
					},
					take,
				}
			}
		] as const,
		queryFn: ({ pageParam: cursor, queryKey }) =>
			ctx.client.query([
				'search.paths',
				{
					...queryKey[1].arg,
					cursor
				}
			]),
		getNextPageParam: (lastPage) => lastPage.cursor ?? undefined
	});

	const items = useMemo(() => query.data?.pages.flatMap((d) => d.items), [query.data]);

	return { query, items };
};

function getLastSectionOfPath(path: string): string | undefined {
	if (path.endsWith('/')) {
		path = path.slice(0, -1);
	}
	const sections = path.split('/');
	const lastSection = sections[sections.length - 1];
	return lastSection;
}
