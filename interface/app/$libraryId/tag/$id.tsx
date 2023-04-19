import { useParams } from 'react-router-dom';
import { useLibraryQuery } from '@sd/client';
import Explorer from '../Explorer';
import { ToolOption } from '../TopBar/ToolBarProvider';
import { useToolBar } from '../TopBar/useToolBar';

export const Component = () => {
	const { id } = useParams<{ id: string }>();

	const explorerData = useLibraryQuery(['tags.getExplorerData', Number(id)]);
	const toolBarOptions: { options: ToolOption[][] } = {
		options: [[]]
	};
	useToolBar(toolBarOptions);

	return (
		<div className="w-full">{explorerData.data && <Explorer data={explorerData.data} />}</div>
	);
};
