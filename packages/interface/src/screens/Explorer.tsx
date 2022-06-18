import { useBridgeQuery } from '@sd/client';
import React from 'react';
import { useParams, useSearchParams } from 'react-router-dom';

import { FileList } from '../components/file/FileList';
import { Inspector } from '../components/file/Inspector';
import { TopBar } from '../components/layout/TopBar';
import { useExplorerState } from '../hooks/useExplorerState';

export const ExplorerScreen: React.FC<{}> = () => {
	let [searchParams] = useSearchParams();
	let path = searchParams.get('path') || '';

	let { id } = useParams();
	let location_id = Number(id);

	const [limit, setLimit] = React.useState(100);

	const { selectedRowIndex } = useExplorerState();

	// Current Location
	const { data: currentLocation } = useBridgeQuery('SysGetLocation', { id: location_id });

	// Current Directory
	const { data: currentDir } = useBridgeQuery(
		'LibGetExplorerDir',
		{ location_id: location_id!, path, limit },
		{ enabled: !!location_id }
	);

	return (
		<div className="flex flex-col w-full h-full">
			<TopBar />
			<div className="relative flex flex-row w-full ">
				<FileList location_id={location_id} path={path} limit={limit} />
				{currentDir?.contents && (
					<Inspector
						location={currentLocation}
						selectedFile={currentDir.contents[selectedRowIndex]}
						locationId={location_id}
					/>
				)}
			</div>
		</div>
	);
};
