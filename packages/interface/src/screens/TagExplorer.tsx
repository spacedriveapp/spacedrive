import { useParams } from 'react-router-dom';
import { useCurrentLibrary, useLibraryQuery } from '@sd/client';
import Explorer from '~/components/explorer/Explorer';

export default function TagExplorer() {
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
}
