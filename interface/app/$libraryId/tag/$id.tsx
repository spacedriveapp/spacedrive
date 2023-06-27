import { Tag } from 'phosphor-react';
import { useLoaderData } from 'react-router';
import { useLibraryQuery } from '@sd/client';
import Explorer from '~/app/$libraryId/Explorer';
import { TopBarPortal } from '~/app/$libraryId/TopBar/Portal';
import TopBarOptions from '~/app/$libraryId/TopBar/TopBarOptions';
import { LocationIdParamsSchema } from '~/app/route-schemas';
import { useExplorerTopBarOptions, useZodRouteParams } from '~/hooks';

export const Component = () => {
	const { id: locationId } = useZodRouteParams(LocationIdParamsSchema);

	const topBarOptions = useExplorerTopBarOptions();

	const explorerData = useLibraryQuery([
		'search.objects',
		{
			filter: {
				tags: [locationId]
			}
		}
	]);

	return (
		<>
			<TopBarPortal
				right={
					<TopBarOptions
						options={[
							topBarOptions.explorerViewOptions,
							topBarOptions.explorerToolOptions,
							topBarOptions.explorerControlOptions
						]}
					/>
				}
			/>
			<Explorer
				items={explorerData.data?.items || null}
				emptyNotice={{
					icon: Tag,
					message: 'No items assigned to this tag'
				}}
			/>
		</>
	);
};
