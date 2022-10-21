import { ComponentMeta, ComponentStory } from '@storybook/react';

import { Root } from './Dropdown';

export default {
	title: 'UI/Dropdown',
	component: Root,
	argTypes: {},
	parameters: {
		backgrounds: {
			default: 'dark'
		}
	}
} as ComponentMeta<typeof Root>;

const Template: ComponentStory<typeof Root> = (args) => <Root {...args} />;

export const Default = Template.bind({});
Default.args = {
	buttonText: 'Item 1',
	items: [
		[
			{
				name: 'Item 1',
				selected: true
			},
			{
				name: 'Item 2',
				selected: false
			}
		]
	]
};
