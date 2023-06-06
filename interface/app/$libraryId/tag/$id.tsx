import { z } from 'zod';
import { useLibraryQuery } from '@sd/client';
import { useZodRouteParams } from '~/hooks';
import Explorer from '../Explorer';

const PARAMS = z.object({
	id: z.coerce.number()
});

export const Component = () => {
	const { id } = useZodRouteParams(PARAMS);

	const explorerData = useLibraryQuery([
		'search.objects',
		{
			filter: {
				tags: [id]
			}
		}
	]);

	return (
		<div className="w-full">
			<Explorer
				items={explorerData.data?.items || null}
				emptyNotice={{
					message: 'No items assigned to this tag'
				}}
			/>
		</div>
	);
};
