import { relativeAliasResolver } from '@sd/config/vite';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';
import { createHtmlPlugin } from 'vite-plugin-html';
import svg from 'vite-plugin-svgr';
import tsconfigPaths from 'vite-tsconfig-paths';
import { name, version } from './package.json';

// https://vitejs.dev/config/
export default defineConfig({
	server: {
		port: 8001
	},
	plugins: [
		tsconfigPaths(),
		react(),
		svg({ svgrOptions: { icon: true } }),
		{
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
		},
		createHtmlPlugin({
			minify: true
		})
	],
	css: {
		modules: {
			localsConvention: 'camelCaseOnly'
		}
	},
	resolve: {
		alias: [relativeAliasResolver]
	},
	root: 'src',
	define: {
		pkgJson: { name, version }
	},
	build: {
		outDir: '../dist',
		assetsDir: '.'
	}
});
