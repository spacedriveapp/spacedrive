import { MagnifyingGlass } from 'phosphor-react';
import { Suspense, memo, useDeferredValue, useMemo } from 'react';
import { type FilePathSearchOrdering, getExplorerItemData, useLibraryQuery } from '@sd/client';
import { type SearchParams, SearchParamsSchema } from '~/app/route-schemas';
import { useZodSearchParams } from '~/hooks';
import Explorer from './Explorer';
import { ExplorerContextProvider } from './Explorer/Context';
import { DefaultTopBarOptions } from './Explorer/TopBarOptions';
import { EmptyNotice } from './Explorer/View';
import {
	createDefaultExplorerSettings,
	filePathOrderingKeysSchema,
	getExplorerStore
} from './Explorer/store';
import { useExplorer, useExplorerSettings } from './Explorer/useExplorer';
import { TopBarPortal } from './TopBar/Portal';

const SearchExplorer = memo((props: { args: SearchParams }) => {
	const { search, ...args } = props.args;

	const query = useLibraryQuery(['search.paths', { ...args, filter: { search } }], {
		suspense: true,
		enabled: !!search,
		onSuccess: () => getExplorerStore().resetNewThumbnails()
	});

	const explorerSettings = useExplorerSettings({
		settings: useMemo(
			() =>
				createDefaultExplorerSettings<FilePathSearchOrdering>({
					order: {
						field: 'name',
						value: 'Asc'
					}
				}),
			[]
		),
		orderingKeys: filePathOrderingKeysSchema
	});

	const settingsSnapshot = explorerSettings.useSettingsSnapshot();

	const items = useMemo(() => {
		const items = query.data?.items ?? [];

		if (settingsSnapshot.layoutMode !== 'media') return items;

		return items?.filter((item) => {
			const { kind } = getExplorerItemData(item);
			return kind === 'Video' || kind === 'Image';
		});
	}, [query.data, settingsSnapshot.layoutMode]);

	const explorer = useExplorer({
		items,
		settings: explorerSettings
	});

	return (
		<ExplorerContextProvider explorer={explorer}>
			<TopBarPortal right={<DefaultTopBarOptions />} />
			<Explorer
				emptyNotice={
					<EmptyNotice
						icon={
							!search ? (
								<MagnifyingGlass
									size={110}
									className="mb-5 text-ink-faint"
									opacity={0.3}
								/>
							) : null
						}
						message={
							search ? `No results found for "${search}"` : 'Search for files...'
						}
					/>
				}
			/>
		</ExplorerContextProvider>
	);
});

export const Component = () => {
	const [searchParams] = useZodSearchParams(SearchParamsSchema);

	const search = useDeferredValue(searchParams);

	return (
		<Suspense>
			<SearchExplorer args={search} />
		</Suspense>
	);
};
