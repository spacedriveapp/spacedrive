import { z } from 'zod';
import { useLibraryQuery } from '@sd/client';
import { useExplorerTopBarOptions, useZodRouteParams } from '~/hooks';
import Explorer from '../Explorer';
import { TopBarPortal } from '../TopBar/Portal';
import TopBarOptions from '../TopBar/TopBarOptions';

const PARAMS = z.object({
	id: z.coerce.number()
});

export const Component = () => {
	const { id } = useZodRouteParams(PARAMS);

	const topBarOptions = useExplorerTopBarOptions();

	const explorerData = useLibraryQuery([
		'search.objects',
		{
			filter: {
				tags: [id]
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
			{explorerData.data && <Explorer items={explorerData.data.items} />}
		</>
	);
};
