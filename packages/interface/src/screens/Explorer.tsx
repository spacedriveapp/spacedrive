import { rspc, useExplorerStore, useLibraryQuery, useLibraryStore } from '@sd/client';
import React from 'react';
import { useParams, useSearchParams } from 'react-router-dom';

import { FileList } from '../components/file/FileList';
import { Inspector } from '../components/file/Inspector';
import { TopBar } from '../components/layout/TopBar';

export const ExplorerScreen: React.FC<{}> = () => {
	const [searchParams] = useSearchParams();
	const path = searchParams.get('path') ?? '';

	const { id } = useParams();
	const locationId = Number(id);

	const [limit] = React.useState(100);

	const { selectedRowIndex, addNewThumbnail } = useExplorerStore();

	const libraryId = useLibraryStore((state) => state.currentLibraryUuid);
	// eslint-disable-next-line @typescript-eslint/no-non-null-assertion
	rspc.useSubscription(['jobs.newThumbnail', { library_id: libraryId!, arg: null }], {
		onNext: (casI) => {
			addNewThumbnail(casI);
		}
	});

	// Current Location
	const { data: currentLocation } = useLibraryQuery(['locations.getById', locationId]);

	// Current Directory
	const { data: currentDir } = useLibraryQuery(
		['locations.getExplorerDir', { location_id: locationId, path, limit }],
		{ enabled: !!locationId }
	);

	return (
		<div className="relative flex flex-col w-full bg-gray-650">
			<TopBar />
			<div className="relative flex flex-row w-full max-h-full">
				<FileList location_id={locationId} path={path} limit={limit} />
				{currentDir?.contents && (
					<Inspector
						location={currentLocation}
						selectedFile={currentDir.contents[selectedRowIndex]}
						locationId={locationId}
					/>
				)}
			</div>
		</div>
	);
};
