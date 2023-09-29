import { sentryVitePlugin } from '@sentry/vite-plugin';
import { defineConfig, loadEnv, mergeConfig, Plugin } from 'vite';

import baseConfig from '../../packages/config/vite';

const devtoolsPlugin: Plugin = {
	name: 'devtools-plugin',
	transformIndexHtml(html) {
		const isDev = process.env.NODE_ENV === 'development';
		if (isDev) {
			const devtoolsScript = `<script src="http://localhost:8097"></script>`;
			const headTagIndex = html.indexOf('</head>');
			if (headTagIndex > -1) {
				return html.slice(0, headTagIndex) + devtoolsScript + html.slice(headTagIndex);
			}
		}
		return html;
	}
};

export default defineConfig(({ mode }) => {
	process.env = { ...process.env, ...loadEnv(mode, process.cwd(), '') };

	return mergeConfig(baseConfig, {
		server: {
			port: 8001
		},
		plugins: [
			devtoolsPlugin,
			process.env.SENTRY_AUTH_TOKEN &&
				sentryVitePlugin({
					authToken: process.env.SENTRY_AUTH_TOKEN,
					org: 'spacedriveapp',
					project: 'desktop'
				})
		]
	});
});
