import type { StorybookConfig } from '@storybook/react-vite';

const config: StorybookConfig = {
	stories: [
		{
			directory: '../../../packages/ui/src/**',
			titlePrefix: 'UI',
			files: '*.stories.*'
		},
		{
			directory: '../../../interface/app/**',
			titlePrefix: 'Interface',
			files: '*.stories.*'
		}
	],
	addons: [
		'@storybook/addon-links',
		'@storybook/addon-essentials',
		'@storybook/addon-interactions',
		'@storybook/addon-styling-webpack'
	],
	framework: {
		name: '@storybook/react-vite',
		options: {}
	},
	docs: {
		autodocs: 'tag'
	}
};
export default config;
