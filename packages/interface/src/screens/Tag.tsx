import { ExplorerKind, useLibraryQuery, useLibraryStore } from '@sd/client';
import React from 'react';
import { useParams } from 'react-router-dom';

import Explorer from '../components/file/Explorer';

export const TagScreen: React.FC<unknown> = () => {
	const { id } = useParams();
	const library_id = useLibraryStore((state) => state.currentLibraryUuid);

	const { data: files } = useLibraryQuery(['tags.getFiles', Number(id)]);
	const { data: tag } = useLibraryQuery(['tags.get', Number(id)]);

	return (
		<div className="w-full">
			{/* {JSON.stringify({ tag, files })} */}
			{library_id && id != undefined && (
				<Explorer
					kind={ExplorerKind.Tag}
					library_id={library_id}
					identifier={Number(id)}
					heading={tag?.name || ''}
					// files={files} // TODO: FIX
				/>
			)}
		</div>
	);
};
