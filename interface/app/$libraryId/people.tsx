import { ScreenHeading } from '@sd/ui';
import TopBarChildren from './TopBar/TopBarChildren';

export const Component = () => {
	return (
		<>
			<TopBarChildren toolOptions={[[]]} />
			<ScreenHeading>People</ScreenHeading>
		</>
	);
};
