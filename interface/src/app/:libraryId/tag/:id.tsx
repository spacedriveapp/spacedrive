import { useParams } from 'react-router-dom';
import { useLibraryQuery } from '@sd/client';
import Explorer from '../Explorer';

export default function TagExplorer() {
	const { id } = useParams<{ id: string }>();

	const explorerData = useLibraryQuery(['tags.getExplorerData', Number(id)]);

	return <div className="w-full">{explorerData.data && <Explorer data={explorerData.data} />}</div>;
}
