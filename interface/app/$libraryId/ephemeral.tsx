import { FolderNotchOpen } from 'phosphor-react';
import { Suspense, memo, useDeferredValue, useMemo } from 'react';
import { getExplorerItemData, useLibraryQuery } from '@sd/client';
import { PathParams, PathParamsSchema } from '~/app/route-schemas';
import { useOperatingSystem, useZodSearchParams } from '~/hooks';
import Explorer from './Explorer';
import { ExplorerContext } from './Explorer/Context';
import { DefaultTopBarOptions } from './Explorer/TopBarOptions';
import { EmptyNotice } from './Explorer/View';
import { getExplorerStore, useExplorerStore } from './Explorer/store';
import { TopBarPortal } from './TopBar/Portal';
import { AddLocationButton } from './settings/library/locations/AddLocationButton';

const EphemeralExplorer = memo(({ args: { path } }: { args: PathParams }) => {
	const os = useOperatingSystem();
	const explorerStore = useExplorerStore();

	const query = useLibraryQuery(
		['search.ephemeral-paths', { path: path ?? (os === 'windows' ? 'C:\\' : '/') }],
		{
			enabled: !!path,
			onSuccess: () => getExplorerStore().resetNewThumbnails()
		}
	);

	const items =
		useMemo(() => {
			const items = query.data?.entries;
			if (explorerStore.layoutMode !== 'media') return items;

			return items?.filter((item) => {
				const { kind } = getExplorerItemData(item);
				return kind === 'Video' || kind === 'Image';
			});
		}, [query.data, explorerStore.layoutMode]) ?? [];

	return (
		<ExplorerContext.Provider value={{}}>
			<TopBarPortal
				left={<AddLocationButton className="max-w-[360px] shrink" path={path} />}
				right={<DefaultTopBarOptions />}
				noSearch={true}
			/>
			<Explorer items={items} />
		</ExplorerContext.Provider>
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
