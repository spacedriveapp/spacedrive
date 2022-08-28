/* eslint-disable react-hooks/exhaustive-deps */
import { ExplorerKind, rspc, useExplorerStore, useLibraryQuery, useLibraryStore } from '@sd/client';
import React, { useEffect } from 'react';
import { useParams, useSearchParams } from 'react-router-dom';

import Explorer from '../components/file/Explorer';

export const ExplorerScreen: React.FC<unknown> = () => {
	const [searchParams] = useSearchParams();

	const urlPath = searchParams.get('path') || '';
	const { path, setPath } = useExplorerStore();
	useEffect(() => {
		setPath(urlPath);
	}, [urlPath]);

	const { id } = useParams();
	const location_id = Number(id);

	const library_id = useLibraryStore((state) => state.currentLibraryUuid);

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
