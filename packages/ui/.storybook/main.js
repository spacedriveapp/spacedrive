module.exports = {
	stories: ['../src/**/*.stories.mdx', '../src/**/*.stories.@(js|jsx|ts|tsx)'],
	addons: [
		'@storybook/addon-links',
		'@storybook/addon-essentials',
		'@storybook/addon-interactions',
		'@storybook/preset-scss',
		{
			name: '@storybook/addon-postcss',
			options: {
				postcssLoaderOptions: {
					implementation: require('postcss')
				}
			}
		}
	],
	webpackFinal: async (config) => {
		config.module.rules.push({
			test: /\.scss$/,
			use: ['style-loader', 'css-loader', 'postcss-loader', 'sass-loader']
		});
		config.module.rules.push({
			test: /\.pcss$/,
			use: ['style-loader', 'css-loader', 'postcss-loader']
		});

		return config;
	},
	core: {
		builder: '@storybook/builder-webpack5'
	},
	framework: '@storybook/react'
};
