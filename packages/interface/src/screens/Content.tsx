import { Button } from '@sd/ui';
import React from 'react';

import { WithContextMenu } from '../components/layout/MenuOverlay';

export const ContentScreen: React.FC<unknown> = (props) => {
	// const [address, setAddress] = React.useState('');
	return (
		<div className="flex flex-col w-full h-screen p-5 custom-scroll page-scroll">
			<WithContextMenu menu={[[{ label: 'jeff', children: [[{ label: 'jeff' }]] }]]}>
				<Button variant="gray">Test</Button>
			</WithContextMenu>
		</div>
	);
};
