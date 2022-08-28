import { ExplorerKind, rspc, useExplorerStore, useLibraryQuery, useLibraryStore } from '@sd/client';
import { DirectoryWithContents } from '@sd/core';
import React from 'react';
import { useParams, useSearchParams } from 'react-router-dom';

import { FileList } from '../file/FileList';
import { Inspector } from '../file/Inspector';
import { TopBar } from '../layout/TopBar';

interface Props {
	library_id: string;
	kind: ExplorerKind;
	identifier: number;
	files?: DirectoryWithContents;
}

export default function Explorer(props: Props) {
	const { selectedRowIndex, addNewThumbnail, path, limit } = useExplorerStore();

	rspc.useSubscription(['jobs.newThumbnail', { library_id: props.library_id!, arg: null }], {
		onNext: (cas_id) => {
			addNewThumbnail(cas_id);
		}
	});

	return (
		<div className="relative flex flex-col w-full bg-gray-650">
			<TopBar />
			<div className="relative flex flex-row w-full max-h-full">
				<FileList location_id={props.identifier} path={path} limit={limit} />
				{props.files?.contents && (
					<Inspector
						locationId={props.identifier}
						selectedFile={props.files.contents[selectedRowIndex]}
					/>
				)}
			</div>
		</div>
	);
}
