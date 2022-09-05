/* eslint-disable react-hooks/exhaustive-deps */
import { explorerStore, libraryStore, useLibraryQuery } from '@sd/client';
import React, { useEffect } from 'react';
import { useParams, useSearchParams } from 'react-router-dom';
import { useSnapshot } from 'valtio';

import Explorer from '../components/explorer/Explorer';

export function useExplorerParams() {
	const { id } = useParams();
	const location_id = id ? Number(id) : -1;

	const [searchParams] = useSearchParams();
	const path = searchParams.get('path') || '';
	const limit = Number(searchParams.get('limit')) || 100;

	return { location_id, path, limit };
}

export const LocationExplorer: React.FC<unknown> = () => {
	const { location_id, path } = useExplorerParams();

	useEffect(() => {
		explorerStore.locationId = location_id;
	}, [location_id]);

	const store = useSnapshot(libraryStore);

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
			{store.currentLibraryUuid && explorerData.data && <Explorer data={explorerData.data} />}
		</div>
	);
};
