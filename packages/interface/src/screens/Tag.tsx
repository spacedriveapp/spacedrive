import { ExplorerKind, useLibraryQuery, useLibraryStore } from '@sd/client';
import React from 'react';
import { useParams } from 'react-router-dom';

import Explorer from '../components/file/Explorer';

export const TagScreen: React.FC<unknown> = () => {
	const { id } = useParams();
	const library_id = useLibraryStore((state) => state.currentLibraryUuid);
	const { data: files } = useLibraryQuery(['tags.getFiles', Number(id)]);

	return (
		<div className="w-full p-5">
			{library_id && id && (
				<Explorer
					kind={ExplorerKind.Tag}
					library_id={library_id}
					identifier={Number(id)}
					// files={files} // TODO: FIX
				/>
			)}
		</div>
	);
};
