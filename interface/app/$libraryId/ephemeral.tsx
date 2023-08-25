import { Suspense, memo, useDeferredValue, useMemo } from 'react';
import { type NonIndexedPathOrdering, getExplorerItemData, useLibraryQuery } from '@sd/client';
import { Tooltip } from '@sd/ui';
import { type PathParams, PathParamsSchema } from '~/app/route-schemas';
import { useOperatingSystem, useZodSearchParams } from '~/hooks';
import Explorer from './Explorer';
import { ExplorerContextProvider } from './Explorer/Context';
import { DefaultTopBarOptions } from './Explorer/TopBarOptions';
import {
	createDefaultExplorerSettings,
	getExplorerStore,
	nonIndexedPathOrderingSchema
} from './Explorer/store';
import { useExplorer, useExplorerSettings } from './Explorer/useExplorer';
import { TopBarPortal } from './TopBar/Portal';
import { AddLocationButton } from './settings/library/locations/AddLocationButton';

const EphemeralExplorer = memo((props: { args: PathParams }) => {
	const os = useOperatingSystem();
	const { path } = props.args;

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

	const settingsSnapshot = explorerSettings.useSettingsSnapshot();

	const query = useLibraryQuery(
		[
			'search.ephemeralPaths',
			{
				path: path ?? (os === 'windows' ? 'C:\\' : '/'),
				withHiddenFiles: true,
				order: settingsSnapshot.order
			}
		],
		{
			enabled: path != null,
			suspense: true,
			onSuccess: () => getExplorerStore().resetNewThumbnails()
		}
	);

	const items =
		useMemo(() => {
			const items = query.data?.entries;
			if (settingsSnapshot.layoutMode !== 'media') return items;

			return items?.filter((item) => {
				const { kind } = getExplorerItemData(item);
				return kind === 'Video' || kind === 'Image';
			});
		}, [query.data, settingsSnapshot.layoutMode]) ?? [];

	const explorer = useExplorer({
		items,
		settings: explorerSettings
	});

	return (
		<ExplorerContextProvider explorer={explorer}>
			<TopBarPortal
				left={
					<Tooltip
						label="Add path as an indexed location"
						className="w-max min-w-0 shrink"
					>
						<AddLocationButton path={path} />
					</Tooltip>
				}
				right={<DefaultTopBarOptions />}
				noSearch={true}
			/>
			<Explorer />
		</ExplorerContextProvider>
	);
});

export const Component = () => {
	const [pathParams] = useZodSearchParams(PathParamsSchema);

	const path = useDeferredValue(pathParams);

	return (
		<Suspense>
			<EphemeralExplorer args={path} />
		</Suspense>
	);
};
