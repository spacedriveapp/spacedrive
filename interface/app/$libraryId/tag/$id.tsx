import { getIcon, iconNames } from '@sd/assets/util';
import { useMemo } from 'react';
import { ObjectOrder, useLibraryQuery } from '@sd/client';
import { LocationIdParamsSchema } from '~/app/route-schemas';
import { useZodRouteParams } from '~/hooks';

import Explorer from '../Explorer';
import { ExplorerContextProvider } from '../Explorer/Context';
import { createDefaultExplorerSettings, objectOrderingKeysSchema } from '../Explorer/store';
import { DefaultTopBarOptions } from '../Explorer/TopBarOptions';
import { useExplorer, useExplorerSettings } from '../Explorer/useExplorer';
import { EmptyNotice } from '../Explorer/View';
import { TopBarPortal } from '../TopBar/Portal';

export const Component = () => {
	const { id: tagId } = useZodRouteParams(LocationIdParamsSchema);

	const explorerData = useLibraryQuery([
		'search.objects',
		{
			filter: { tags: [tagId] },
			take: 100
		}
	]);

	const tag = useLibraryQuery(['tags.get', tagId], { suspense: true });

	const explorerSettings = useExplorerSettings({
		settings: useMemo(
			() =>
				createDefaultExplorerSettings<ObjectOrder>({
					order: null
				}),
			[]
		),
		onSettingsChanged: () => {},
		orderingKeys: objectOrderingKeysSchema
	});

	const explorer = useExplorer({
		items: explorerData.data?.items || null,
		settings: explorerSettings,
		...(tag.data && {
			parent: { type: 'Tag', tag: tag.data }
		})
	});

	return (
		<ExplorerContextProvider explorer={explorer}>
			<TopBarPortal right={<DefaultTopBarOptions />} />
			<Explorer
				emptyNotice={
					<EmptyNotice
						loading={explorerData.isFetching}
						icon={<img className="h-32 w-32" src={getIcon(iconNames.Tags)} />}
						message="No items assigned to this tag."
					/>
				}
			/>
		</ExplorerContextProvider>
	);
};
