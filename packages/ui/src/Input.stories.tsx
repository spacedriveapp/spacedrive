import { Meta } from '@storybook/react';
import { useState } from 'react';

import { Input } from './Input';

const meta: Meta<typeof Input> = {
	title: 'Input',
	component: Input,
	argTypes: {},
	parameters: {
		backgrounds: {
			default: 'dark'
		}
	},
	args: {
		children: 'Input'
	}
};

export default meta;

export const Default = () => {
	const [value, setValue] = useState('Spacedrive');

	return (
		<div className="flex w-48 flex-col bg-app p-8">
			<Input value={value} onChange={(e) => setValue(e.target.value)} />
		</div>
	);
};
