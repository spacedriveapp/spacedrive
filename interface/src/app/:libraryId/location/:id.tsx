import { useEffect } from 'react';
import { useParams, useSearchParams } from 'react-router-dom';
import { useLibraryQuery } from '@sd/client';
import Explorer from '~/components/Explorer';
import { getExplorerStore } from '~/hooks/useExplorerStore';

export function useExplorerParams() {
	const { id } = useParams<{ id?: string }>();
	const location_id = id ? Number(id) : null;

	const [searchParams] = useSearchParams();
	const path = searchParams.get('path') || '';
	const limit = Number(searchParams.get('limit')) || 100;

	return { location_id, path, limit };
}

export default () => {
	const { location_id, path } = useExplorerParams();

	useEffect(() => {
		getExplorerStore().locationId = location_id;
	}, [location_id]);

	if (location_id === null) throw new Error(`location_id is null!`);

	const explorerData = useLibraryQuery([
		'locations.getExplorerData',
		{
			location_id,
			path: path,
			limit: 100,
			cursor: null
		}
	]);

	return (
		<div className="relative flex w-full flex-col">
			<Explorer data={explorerData.data} />
		</div>
	);
};
