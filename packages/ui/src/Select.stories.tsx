import { Meta } from '@storybook/react';
import { useState } from 'react';
import { Select, SelectOption } from './Select';

export default {
	title: 'Select',
	component: Select,
	argTypes: {},
	parameters: {
		backgrounds: {
			default: 'dark'
		}
	},
	args: {
		children: 'Select'
	}
} as Meta<typeof Select>;

export const Default = () => {
	const VALUES = ['Option 1', 'Option 2', 'Option 3'] as const;

	const [value, setValue] = useState(VALUES[0]);

	return (
		<div className="flex w-48 flex-col bg-app p-8">
			<Select value={value} onChange={setValue}>
				{VALUES.map((value) => (
					<SelectOption value={value} key={value}>
						{value}
					</SelectOption>
				))}
			</Select>
		</div>
	);
};
