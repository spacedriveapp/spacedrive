/* eslint-disable react-hooks/exhaustive-deps */
import { ExplorerKind, rspc, useExplorerStore, useLibraryQuery, useLibraryStore } from '@sd/client';
import React, { useEffect } from 'react';
import { useParams, useSearchParams } from 'react-router-dom';

import Explorer from '../components/file/Explorer';

export const ExplorerScreen: React.FC<unknown> = () => {
	const { id } = useParams();
	const location_id = Number(id);
	const [searchParams] = useSearchParams();
	const pathParam = searchParams.get('path') || '';
	const limitParam = Number(searchParams.get('limit')) || 100;

	const { path, set, limit } = useExplorerStore();
	const library_id = useLibraryStore((state) => state.currentLibraryUuid);

	// update explorer store when screen's path param changes
	useEffect(() => {
		set({ path: pathParam });
	}, [pathParam]);

	// same for limit
	useEffect(() => {
		set({ limit: limitParam });
	}, [limitParam]);

	const { data: files } = useLibraryQuery([
		'locations.getExplorerDir',
		{
			location_id: location_id,
			path: path,
			limit: 100
		}
	]);

	return (
		<div className="relative flex flex-col w-full">
			{library_id && (
				<Explorer
					library_id={library_id}
					kind={ExplorerKind.Location}
					identifier={location_id}
					files={files}
				/>
			)}
		</div>
	);
};
