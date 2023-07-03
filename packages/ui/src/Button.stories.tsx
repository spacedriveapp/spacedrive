import { Meta, StoryFn } from '@storybook/react';
import { Button } from './Button';

const meta: Meta<typeof Button> = {
	title: 'Button',
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
};

export default meta;

const Template: StoryFn<typeof Button> = (args) => <Button {...args} />;

export const Default: StoryFn<typeof Button> = Template.bind({});
Default.args = {
	variant: 'default'
};

export const Primary: StoryFn<typeof Button> = Template.bind({});
Primary.args = {
	variant: 'accent'
};

export const PrimarySmall: StoryFn<typeof Button> = Template.bind({});
PrimarySmall.args = {
	variant: 'accent',
	size: 'sm'
};
