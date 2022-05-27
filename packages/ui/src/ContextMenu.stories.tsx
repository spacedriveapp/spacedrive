import { ComponentMeta, ComponentStory } from '@storybook/react';
import { FileText, Plus, Trash } from 'phosphor-react';
import React from 'react';

import { ContextMenu } from './ContextMenu';

export default {
	title: 'UI/Context Menu',
	component: ContextMenu,
	argTypes: {},
	parameters: {},
	args: {}
} as ComponentMeta<typeof ContextMenu>;

const Template: ComponentStory<typeof ContextMenu> = (args) => <ContextMenu {...args} />;

export const Default = Template.bind({});
Default.args = {
	sections: [
		{
			items: [
				{
					label: 'New Item',
					icon: Plus,
					onClick: () => alert('Item clicked')
				}
			]
		},
		{
			items: [
				{
					label: 'View Info',
					icon: FileText,
					onClick: () => alert('Info!!!')
				},
				{
					label: 'Delete',
					icon: Trash,
					danger: true,
					onClick: () => alert('Delete item clicked')
				}
			]
		}
	]
};
