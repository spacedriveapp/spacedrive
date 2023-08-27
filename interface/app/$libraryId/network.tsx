import { Globe } from '@sd/assets/icons';
import { Suspense, memo, useDeferredValue, useMemo } from 'react';
import { type NonIndexedPathOrdering } from '@sd/client';
import { type PathParams, PathParamsSchema } from '~/app/route-schemas';
import { useOperatingSystem, useZodSearchParams } from '~/hooks';
import Explorer from './Explorer';
import { ExplorerContextProvider } from './Explorer/Context';
import { DefaultTopBarOptions } from './Explorer/TopBarOptions';
import { createDefaultExplorerSettings, nonIndexedPathOrderingSchema } from './Explorer/store';
import { useExplorer, useExplorerSettings } from './Explorer/useExplorer';
import { TopBarPortal } from './TopBar/Portal';

const Network = memo((props: { args: PathParams }) => {
	const os = useOperatingSystem();

	const explorerSettings = useExplorerSettings({
		settings: useMemo(
			() =>
				createDefaultExplorerSettings<NonIndexedPathOrdering>({
					order: {
						field: 'name',
						value: 'Asc'
					}
				}),
			[]
		),
		orderingKeys: nonIndexedPathOrderingSchema
	});

	const explorer = useExplorer({
		items: [],
		settings: explorerSettings
	});

	return (
		<ExplorerContextProvider explorer={explorer}>
			<TopBarPortal
				left={
					<div className="flex items-center gap-2">
						<img src={Globe} className="mt-[-1px] h-[22px] w-[22px]" />
						<span className="truncate text-sm font-medium">Network</span>
					</div>
				}
				right={<DefaultTopBarOptions />}
				noSearch={true}
			/>
			<Explorer
				emptyNotice={
					<div className="flex h-full flex-col items-center justify-center text-white">
						<img src={Globe} className="h-32 w-32" />
						<h1 className="mt-4 text-lg font-bold">Your Local Network</h1>
						<p className="mt-1 text-sm text-ink-dull">
							You don't have anything in your network yet.
						</p>
					</div>
				}
			/>
		</ExplorerContextProvider>
	);
});

export const Component = () => {
	const [pathParams] = useZodSearchParams(PathParamsSchema);
	const path = useDeferredValue(pathParams);

	return (
		<Suspense>
			<Network args={path} />
		</Suspense>
	);
};
