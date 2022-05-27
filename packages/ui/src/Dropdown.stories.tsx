import { ViewListIcon } from '@heroicons/react/solid';
import { ComponentMeta, ComponentStory } from '@storybook/react';
import React from 'react';

import { Dropdown } from './Dropdown';

export default {
	title: 'UI/Dropdown',
	component: Dropdown,
	argTypes: {},
	parameters: {
		backgrounds: {
			default: 'dark'
		}
	}
} as ComponentMeta<typeof Dropdown>;

const Template: ComponentStory<typeof Dropdown> = (args) => <Dropdown {...args} />;

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
