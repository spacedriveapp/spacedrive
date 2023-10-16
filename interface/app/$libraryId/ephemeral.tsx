import { getIcon, iconNames } from '@sd/assets/util';
import { memo, Suspense, useDeferredValue, useMemo } from 'react';
import {
	ExplorerItem,
	getExplorerItemData,
	useLibraryQuery,
	type EphemeralPathOrder
} from '@sd/client';
import { Tooltip } from '@sd/ui';
import { PathParamsSchema, type PathParams } from '~/app/route-schemas';
import { useIsDark, useOperatingSystem, useZodSearchParams } from '~/hooks';

import Explorer from './Explorer';
import { ExplorerContextProvider } from './Explorer/Context';
import {
	createDefaultExplorerSettings,
	getExplorerStore,
	nonIndexedPathOrderingSchema
} from './Explorer/store';
import { DefaultTopBarOptions } from './Explorer/TopBarOptions';
import { useExplorer, useExplorerSettings } from './Explorer/useExplorer';
import { EmptyNotice } from './Explorer/View';
import { AddLocationButton } from './settings/library/locations/AddLocationButton';
import { TopBarPortal } from './TopBar/Portal';

const EphemeralExplorer = memo((props: { args: PathParams }) => {
	const os = useOperatingSystem();
	const isDark = useIsDark();
	const { path } = props.args;

	const explorerSettings = useExplorerSettings({
		settings: useMemo(
			() =>
				createDefaultExplorerSettings<EphemeralPathOrder>({
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
				withHiddenFiles: false,
				order: settingsSnapshot.order
			}
		],
		{
			enabled: path != null,
			suspense: true,
			onSuccess: () => getExplorerStore().resetNewThumbnails()
		}
	);

	const items = useMemo(() => {
		if (!query.data) return [];

		const ret: ExplorerItem[] = [];

		for (const item of query.data.entries) {
			if (
				!settingsSnapshot.showHiddenFiles &&
				item.type === 'NonIndexedPath' &&
				item.item.hidden
			)
				continue;

			if (settingsSnapshot.layoutMode !== 'media') ret.push(item);
			else {
				const { kind } = getExplorerItemData(item);

				if (kind === 'Video' || kind === 'Image') ret.push(item);
			}
		}

		return ret;
	}, [query.data, settingsSnapshot.layoutMode, settingsSnapshot.showHiddenFiles]);

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
			<Explorer
				emptyNotice={
					<EmptyNotice
						loading={query.isFetching}
						icon={
							<img
								className="h-32 w-32"
								src={getIcon(iconNames.FolderNoSpace, isDark)}
							/>
						}
						message="No files found here"
					/>
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
			<EphemeralExplorer args={path} />
		</Suspense>
	);
};
