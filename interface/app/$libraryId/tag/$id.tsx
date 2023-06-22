import { z } from 'zod';
import { useLibraryQuery } from '@sd/client';
import { useExplorerTopBarOptions, useZodRouteParams } from '~/hooks';
import Explorer from '../Explorer';
import { TopBarPortal } from '../TopBar/Portal';
import TopBarOptions from '../TopBar/TopBarOptions';

export const Component = () => {
	const { id: locationId } = useZodRouteParams();

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
					message: 'No items assigned to this tag'
				}}
			/>
		</>
	);
};
