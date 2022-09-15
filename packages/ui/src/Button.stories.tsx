import { ComponentMeta, ComponentStory } from '@storybook/react';

import { Button } from './Button';

export default {
	title: 'UI/Button',
	component: Button,
	argTypes: {},
	parameters: {
		backgrounds: {
			default: 'dark'
		}
	},
	args: {
		children: 'Button'
	}
} as ComponentMeta<typeof Button>;

const Template: ComponentStory<typeof Button> = (args) => <Button {...args} />;

export const Default = Template.bind({});
Default.args = {
	variant: 'default'
};

export const Primary = Template.bind({});
Primary.args = {
	variant: 'primary'
};

export const PrimarySmall = Template.bind({});
PrimarySmall.args = {
	variant: 'primary',
	size: 'sm'
};
