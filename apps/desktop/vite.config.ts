import { Plugin, mergeConfig } from 'vite';
import { comlink } from 'vite-plugin-comlink';
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

export default mergeConfig(baseConfig, {
	server: {
		port: 8001
	},
	plugins: [devtoolsPlugin, comlink()],
	worker: {
		plugins: [comlink()]
	}
});
