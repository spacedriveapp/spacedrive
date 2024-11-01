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
		build: {
			rollupOptions: {
				treeshake: 'recommended',
				external: [
					// Don't bundle Fda video for non-macOS platforms
					process.platform !== 'darwin' && /^@sd\/assets\/videos\/Fda.mp4$/
				].filter(Boolean)
			}
		},
		plugins: [
			devtoolsPlugin,
			process.env.SENTRY_AUTH_TOKEN &&
				// All this plugin does is give Sentry access to source maps and release data for errors that users *choose* to report
				sentryVitePlugin({
					authToken: process.env.SENTRY_AUTH_TOKEN,
					org: 'spacedriveapp',
					project: 'desktop'
				})
		]
	});
});
