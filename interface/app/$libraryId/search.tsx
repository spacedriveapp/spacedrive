import { MagnifyingGlass } from 'phosphor-react';
import { Suspense, memo, useDeferredValue, useEffect, useMemo } from 'react';
import { useSearchParams } from 'react-router-dom';
import { z } from 'zod';
import { useLibraryQuery } from '@sd/client';
import { getExplorerStore, useExplorerStore } from '~/hooks/useExplorerStore';
import { useToolOptions } from '~/hooks/useToolOptions';
import Explorer from './Explorer';
import { getExplorerItemData } from './Explorer/util';
import TopBarChildren from './TopBar/TopBarChildren';

const schema = z.object({
	search: z.string().optional(),
	take: z.number().optional(),
	order: z.union([z.object({ name: z.boolean() }), z.object({ name: z.boolean() })]).optional()
});

export type SearchArgs = z.infer<typeof schema>;

const ExplorerStuff = memo((props: { args: SearchArgs }) => {
	const explorerStore = useExplorerStore();
	const { explorerViewOptions, explorerControlOptions } = useToolOptions();

	const query = useLibraryQuery(['search', props.args], {
		suspense: true,
		enabled: !!props.args.search
	});

	const items = useMemo(() => {
		const queryData = query.data;
		if (explorerStore.layoutMode !== 'media') return queryData;

		const mediaItems = queryData?.filter((item) => {
			const { kind } = getExplorerItemData(item);
			return kind === 'Video' || kind === 'Image';
		});
		return mediaItems;
	}, [query.data, explorerStore.layoutMode]);

	useEffect(() => {
		getExplorerStore().selectedRowIndex = -1;
	}, [props.args.search]);

	return (
		<>
			{items && items.length > 0 ? (
				<>
					<TopBarChildren toolOptions={[explorerViewOptions, explorerControlOptions]} />
					<Explorer items={items} />
				</>
			) : (
				<div className="flex flex-1 flex-col items-center justify-center">
					{!props.args.search && (
						<MagnifyingGlass size={110} className="mb-5 text-ink-faint" opacity={0.3} />
					)}
					<p className="text-xs text-ink-faint">
						{props.args.search
							? `No results found for "${props.args.search}"`
							: 'Search for files...'}
					</p>
				</div>
			)}
		</>
	);
});

export const Component = () => {
	const [searchParams] = useSearchParams();

	const searchObj = useMemo(
		() => schema.parse(Object.fromEntries([...searchParams])),
		[searchParams]
	);

	const search = useDeferredValue(searchObj);

	return (
		<Suspense fallback="LOADING FIRST RENDER">
			<ExplorerStuff args={search} />
		</Suspense>
	);
};
