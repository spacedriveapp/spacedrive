import { ExplorerKind, useLibraryQuery, useLibraryStore } from '@sd/client';
import React from 'react';
import { useParams } from 'react-router-dom';

import Explorer from '../components/explorer/Explorer';

export const TagScreen: React.FC<unknown> = () => {
	const { id } = useParams();
	const library_id = useLibraryStore((state) => state.currentLibraryUuid);

	const explorerData = useLibraryQuery(['tags.getExplorerData', Number(id)]);
	const { data: tag } = useLibraryQuery(['tags.get', Number(id)]);

	return (
		<div className="w-full">
			{library_id && id != undefined && explorerData.data && <Explorer data={explorerData.data} />}
		</div>
	);
};
