import { useCurrentLibrary, useLibraryQuery } from '@sd/client';
import React from 'react';
import { useParams } from 'react-router-dom';

import Explorer from '../components/explorer/Explorer';

export const TagExplorer: React.FC<unknown> = () => {
	const { id } = useParams();
	const { library } = useCurrentLibrary();

	const explorerData = useLibraryQuery(['tags.getExplorerData', Number(id)]);

	return (
		<div className="w-full">
			{library!.uuid && id != undefined && explorerData.data && (
				<Explorer data={explorerData.data} />
			)}
		</div>
	);
};
