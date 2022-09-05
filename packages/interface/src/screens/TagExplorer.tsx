import { libraryStore, useLibraryQuery } from '@sd/client';
import React from 'react';
import { useParams } from 'react-router-dom';
import { useSnapshot } from 'valtio';

import Explorer from '../components/explorer/Explorer';

export const TagExplorer: React.FC<unknown> = () => {
	const { id } = useParams();
	const store = useSnapshot(libraryStore);

	const explorerData = useLibraryQuery(['tags.getExplorerData', Number(id)]);
	// const { data: tag } = useLibraryQuery(['tags.get', Number(id)]);

	return (
		<div className="w-full">
			{store.currentLibraryUuid && id != undefined && explorerData.data && (
				<Explorer data={explorerData.data} />
			)}
		</div>
	);
};
