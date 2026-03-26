import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";

export default defineConfig({
	plugins: [react()],
	resolve: {
		dedupe: ["react", "react-dom"],
		alias: [
			{
				find: /^react$/,
				replacement: path.resolve(__dirname, "./node_modules/react/index.js"),
			},
			{
				find: /^react\/jsx-runtime$/,
				replacement: path.resolve(__dirname, "./node_modules/react/jsx-runtime.js"),
			},
			{
				find: /^react\/jsx-dev-runtime$/,
				replacement: path.resolve(__dirname, "./node_modules/react/jsx-dev-runtime.js"),
			},
			{
				find: /^react-dom$/,
				replacement: path.resolve(__dirname, "./node_modules/react-dom/index.js"),
			},
			{
				find: /^react-dom\/client$/,
				replacement: path.resolve(__dirname, "./node_modules/react-dom/client.js"),
			},
			{
				find: "@spaceui/tokens/css/themes",
				replacement: path.resolve(
					__dirname,
					"../../../spaceui/packages/tokens/src/css/themes",
				),
			},
			{
				find: "@spaceui/tokens/css",
				replacement: path.resolve(
					__dirname,
					"../../../spaceui/packages/tokens/src/css/base.css",
				),
			},
			{
				find: "@spaceui/tokens",
				replacement: path.resolve(
					__dirname,
					"../../../spaceui/packages/tokens/src/index.ts",
				),
			},
		],
	},
	server: {
		port: 3000,
		fs: {
			allow: [
				path.resolve(__dirname, "../../.."),
				path.resolve(__dirname, "../../../spaceui"),
			],
		},
		proxy: {
			// Proxy RPC requests to server
			"/rpc": {
				target: "http://localhost:8080",
				changeOrigin: true,
			},
		},
	},
	optimizeDeps: {
		exclude: ["@spaceui/ai", "@spaceui/primitives", "@spaceui/tokens"],
	},
	build: {
		outDir: "dist",
		emptyOutDir: true,
		sourcemap: true,
	},
});
