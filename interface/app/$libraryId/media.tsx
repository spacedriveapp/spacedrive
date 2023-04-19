import { ScreenHeading } from '@sd/ui';
import { useToolBar } from './TopBar/useToolBar';

export const Component = () => {
	useToolBar({
		options: [[]]
	});
	return <ScreenHeading>Media</ScreenHeading>;
};
