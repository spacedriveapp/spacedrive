import { Meta, StoryFn } from '@storybook/react';

import { Root } from './Dropdown';

const meta: Meta<typeof Root> = {
	title: 'Dropdown',
	component: Root,
	argTypes: {},
	parameters: {
		backgrounds: {
			default: 'dark'
		}
	}
};

export default meta;

const Template: StoryFn<typeof Root> = (args) => <Root {...args} />;

export const Default: StoryFn<typeof Root> = Template.bind({});

// Default.args = {
// 	buttonText: 'Item 1',
// 	items: [
// 		[
// 			{
// 				name: 'Item 1',
// 				selected: true
// 			},
// 			{
// 				name: 'Item 2',
// 				selected: false
// 			}
// 		]
// 	]
// };
