import { getExplorerStore, useCurrentLibrary, useLibraryQuery } from '@sd/client';
import { useEffect } from 'react';
import { useParams, useSearchParams } from 'react-router-dom';

import Explorer from '../components/explorer/Explorer';

export function useExplorerParams() {
	const { id } = useParams();
	const location_id = id ? Number(id) : -1;

	const [searchParams] = useSearchParams();
	const path = searchParams.get('path') || '';
	const limit = Number(searchParams.get('limit')) || 100;

	return { location_id, path, limit };
}

export default function LocationExplorer() {
	const { location_id, path } = useExplorerParams();
	const { library } = useCurrentLibrary();

	useEffect(() => {
		getExplorerStore().locationId = location_id;
	}, [location_id]);

	const explorerData = useLibraryQuery([
		'locations.getExplorerData',
		{
			location_id: location_id,
			path: path,
			limit: 100,
			cursor: null
		}
	]);

	return (
		<div className="relative flex flex-col w-full">
			<Explorer data={explorerData.data} />
		</div>
	);
}
