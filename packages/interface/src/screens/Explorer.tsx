/* eslint-disable react-hooks/exhaustive-deps */
import { ExplorerKind, rspc, useExplorerStore, useLibraryQuery, useLibraryStore } from '@sd/client';
import React, { useEffect } from 'react';
import { useParams, useSearchParams } from 'react-router-dom';
import z from 'zod';

import Explorer from '../components/explorer/Explorer';

export function useExplorerParams() {
	const { id } = useParams();
	const location_id = id ? Number(id) : -1;

	const [searchParams] = useSearchParams();
	const path = searchParams.get('path') || '';
	const limit = Number(searchParams.get('limit')) || 100;

	return { location_id, path, limit };
}

export const ExplorerScreen: React.FC<unknown> = () => {
	const { location_id, path } = useExplorerParams();

	// for top bar location context, could be replaced with react context as it is child component
	const { set } = useExplorerStore();
	useEffect(() => {
		set({ locationId: location_id });
	}, [location_id]);

	const library_id = useLibraryStore((state) => state.currentLibraryUuid);

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
			{library_id && explorerData.data && <Explorer data={explorerData.data} />}
		</div>
	);
};
