import { ScreenHeading } from '@sd/ui';
import { ToolOption } from './TopBar/ToolBarProvider';
import { useToolBar } from './TopBar/useToolBar';

export const Component = () => {
	const toolBarOptions: { options: ToolOption[][] } = {
		options: [[]]
	};
	useToolBar(toolBarOptions);
	return <ScreenHeading>Media</ScreenHeading>;
};
