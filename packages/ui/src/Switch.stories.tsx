import { Meta } from '@storybook/react';

import { Switch } from './Switch';

const meta: Meta<typeof Switch> = {
	title: 'Switch',
	component: Switch,
	argTypes: {},
	parameters: {
		backgrounds: {
			default: 'dark'
		}
	}
};

export default meta;

export const Default = () => {
	return (
		<div className="h-screen w-full bg-app p-10">
			<h1 className="text-[20px] font-bold text-white">Switch</h1>
			<div className="mt-6">
				<Switch />
			</div>
		</div>
	);
};
