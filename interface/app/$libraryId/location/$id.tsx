import {
	ExplorerItem,
	useLibraryContext,
	useLibraryQuery,
	useLibrarySubscription,
	useRspcLibraryContext
} from '@sd/client';
import { Folder } from '~/components/Folder';
import { useInfiniteQuery } from '@tanstack/react-query';
import { useEffect, useMemo } from 'react';
import { z } from 'zod';
import {
	getExplorerStore,
	useExplorerStore,
	useExplorerTopBarOptions,
	useKeyDeleteFile,
	useZodRouteParams
} from '~/hooks';
import Explorer from '../Explorer';
import { useExplorerOrder, useExplorerSearchParams } from '../Explorer/util';
import { TopBarPortal } from '../TopBar/Portal';
import TopBarOptions from '../TopBar/TopBarOptions';
import LocationOptions from './LocationOptions';

const PARAMS = z.object({
	id: z.coerce.number()
});

export const Component = () => {
	const [{ path }] = useExplorerSearchParams();
	const { id: location_id } = useZodRouteParams(PARAMS);
	const { explorerViewOptions, explorerControlOptions, explorerToolOptions } =
		useExplorerTopBarOptions();

	const location = useLibraryQuery(['locations.get', location_id]);

	useLibrarySubscription(
		[
			'locations.quickRescan',
			{
				location_id,
				sub_path: path ?? ''
			}
		],
		{ onData() { } }
	);

	const explorerStore = getExplorerStore();

	useEffect(() => {
		explorerStore.locationId = location_id;
	}, [explorerStore, location_id, path]);

	const { items, loadMore } = useItems();

	return (
		<>
			<TopBarPortal
				left={
					<div className='group flex flex-row items-center space-x-2'>
						<span>
							<Folder size={22} className="ml-3 mr-2 mt-[-1px] inline-block" />
							<span className="text-sm font-medium">
								{path ? getLastSectionOfPath(path) : location.data?.name}
							</span>
						</span>
						{location.data && <LocationOptions location={location.data} path={path || ""} />}
					</div>
				}
				right={
					<TopBarOptions
						options={[explorerViewOptions, explorerToolOptions, explorerControlOptions]}
					/>
				}
			/>

			<Explorer items={items} onLoadMore={loadMore} />
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
					take
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
		getNextPageParam: (lastPage) => lastPage.cursor ?? undefined,
		keepPreviousData: true,
		onSuccess: () => getExplorerStore().resetNewThumbnails()
	});

	const items = useMemo(() => query.data?.pages.flatMap((d) => d.items) || null, [query.data]);

	function loadMore() {
		if (query.hasNextPage && !query.isFetchingNextPage) {
			query.fetchNextPage();
		}
	}

	return { query, items, loadMore };
};

function getLastSectionOfPath(path: string): string | undefined {
	if (path.endsWith('/')) {
		path = path.slice(0, -1);
	}
	const sections = path.split('/');
	const lastSection = sections[sections.length - 1];
	return lastSection;
}
