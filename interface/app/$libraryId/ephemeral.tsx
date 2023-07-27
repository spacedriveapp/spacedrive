import { Suspense, memo, useDeferredValue, useMemo } from 'react';
import { getExplorerItemData, useLibraryQuery } from '@sd/client';
import { PathParams, PathParamsSchema } from '~/app/route-schemas';
import { useOperatingSystem, useZodSearchParams } from '~/hooks';
import Explorer from './Explorer';
import { ExplorerContext } from './Explorer/Context';
import { DefaultTopBarOptions } from './Explorer/TopBarOptions';
import { getExplorerStore, useExplorerStore } from './Explorer/store';
import { TopBarPortal } from './TopBar/Portal';

const EphemeralExplorer = memo(({ args: { path } }: { args: PathParams }) => {
	const os = useOperatingSystem();
	const explorerStore = useExplorerStore();

	const query = useLibraryQuery(
		['search.ephemeral-paths', { path: path ?? (os === 'windows' ? 'C:\\' : '/') }],
		{
			enabled: !!path,
			suspense: true,
			onSuccess: () => getExplorerStore().resetNewThumbnails()
		}
	);

	const items = useMemo(() => {
		const items = query.data?.entries;
		if (explorerStore.layoutMode !== 'media') return items;

		return items?.filter((item) => {
			const { kind } = getExplorerItemData(item);
			return kind === 'Video' || kind === 'Image';
		});
	}, [query.data, explorerStore.layoutMode]);

	return (
		<>
			{items && items.length > 0 ? (
				<ExplorerContext.Provider value={{}}>
					<TopBarPortal right={<DefaultTopBarOptions />} />
					<Explorer items={items} />
				</ExplorerContext.Provider>
			) : (
				<div className="flex flex-1 flex-col items-center justify-center">
					<p className="text-xs text-ink-faint">
						We're past the event horizon, there are not files here...
					</p>
				</div>
			)}
		</>
	);
});

export const Component = () => {
	const [searchParams] = useZodSearchParams(PathParamsSchema);

	const search = useDeferredValue(searchParams);

	return (
		<Suspense fallback="LOADING FIRST RENDER">
			<EphemeralExplorer args={search} />
		</Suspense>
	);
};
