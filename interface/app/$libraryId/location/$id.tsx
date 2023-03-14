import { useEffect } from 'react';
import { useParams, useSearchParams } from 'react-router-dom';
import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { getExplorerStore } from '~/hooks/useExplorerStore';
import Explorer from '../Explorer';

export function useExplorerParams() {
	const { id } = useParams<{ id?: string }>();
	const location_id = id ? Number(id) : null;

	const [searchParams] = useSearchParams();
	const path = searchParams.get('path') || '';
	const limit = Number(searchParams.get('limit')) || 100;

	return { location_id, path, limit };
}

export default () => {
	const { location_id, path, limit } = useExplorerParams();

	const quickRescan = useLibraryMutation('locations.quickRescan');
	const explorerState = getExplorerStore();

	useEffect(() => {
		explorerState.locationId = location_id;
		if (location_id !== null) quickRescan.mutate({ location_id, sub_path: path });
	}, [location_id, path]);

	if (location_id === null) throw new Error(`location_id is null!`);

	const explorerData = useLibraryQuery([
		'locations.getExplorerData',
		{
			location_id,
			path,
			limit,
			cursor: null
		}
	]);

	return (
		<div className="relative flex w-full flex-col">
			<Explorer data={explorerData.data} />
		</div>
	);
};
