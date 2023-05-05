import { z } from 'zod';
import { useLibraryQuery } from '@sd/client';
import { useZodRouteParams } from '~/hooks';
import Explorer from '../Explorer';

const PARAMS = z.object({
	id: z.coerce.number()
});

export const Component = () => {
	const { id } = useZodRouteParams(PARAMS);

	const explorerData = useLibraryQuery(['tags.getExplorerData', id]);

	return (
		<div className="w-full">
			{explorerData.data && <Explorer items={explorerData.data.items} />}
		</div>
	);
};
