import { useInfiniteQuery } from '@tanstack/react-query';
import { useMemo } from 'react';
import {
	useLibraryContext,
	useLibraryQuery,
	useLibrarySubscription,
	useRspcLibraryContext
} from '@sd/client';
import { LocationIdParamsSchema } from '~/app/route-schemas';
import { Folder } from '~/components';
import { useZodRouteParams } from '~/hooks';
import Explorer from '../Explorer';
import { ExplorerContext } from '../Explorer/Context';
import ContextMenu, { FilePathItems } from '../Explorer/ContextMenu';
import { DefaultTopBarOptions } from '../Explorer/TopBarOptions';
import { getExplorerStore, useExplorerStore } from '../Explorer/store';
import { useExplorerOrder, useExplorerSearchParams } from '../Explorer/util';
import { TopBarPortal } from '../TopBar/Portal';
import LocationOptions from './LocationOptions';

export const Component = () => {
	const [{ path }] = useExplorerSearchParams();
	const { id: locationId } = useZodRouteParams(LocationIdParamsSchema);

	const location = useLibraryQuery(['locations.get', locationId]);

	useLibrarySubscription(
		[
			'locations.quickRescan',
			{
				sub_path: path ?? '',
				location_id: locationId
			}
		],
		{ onData() {} }
	);

	const { items, loadMore } = useItems({ locationId });

	return (
		<ExplorerContext.Provider
			value={{
				parent: location.data
					? {
							type: 'Location',
							location: location.data
					  }
					: undefined
			}}
		>
			<TopBarPortal
				left={
					<div className="group flex flex-row items-center space-x-2">
						<span className="flex flex-row items-center">
							<Folder size={22} className="ml-3 mr-2 mt-[-1px] inline-block" />
							<span className="max-w-[100px] truncate text-sm font-medium">
								{path && path?.length > 1
									? getLastSectionOfPath(path)
									: location.data?.name}
							</span>
						</span>
						{location.data && (
							<LocationOptions location={location.data} path={path || ''} />
						)}
					</div>
				}
				right={<DefaultTopBarOptions />}
			/>

			<Explorer
				items={items}
				onLoadMore={loadMore}
				contextMenu={(item) => (
					<ContextMenu
						item={item}
						extra={({ filePath }) => (
							<>
								{filePath && location.data && (
									<FilePathItems.CutCopyItems
										locationId={location.data.id}
										filePath={filePath}
									/>
								)}
							</>
						)}
					/>
				)}
			/>
		</ExplorerContext.Provider>
	);
};

const useItems = ({ locationId }: { locationId: number }) => {
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
