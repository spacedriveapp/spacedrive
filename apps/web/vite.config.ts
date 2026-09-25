import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "path";
import fs from "fs";

const spaceui = path.resolve(__dirname, "../../../spaceui/packages");
const hasSpaceui = fs.existsSync(spaceui);

export default defineConfig({
	plugins: [react(), tailwindcss()],
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
			// SpaceUI — resolve to source for HMR when available locally
			...(hasSpaceui
				? [
						{
							find: /^@spacedrive\/tokens\/css\/themes\/(.+)$/,
							replacement: `${spaceui}/tokens/src/css/themes/$1.css`,
						},
						{
							find: /^@spacedrive\/tokens\/theme$/,
							replacement: `${spaceui}/tokens/src/css/theme.css`,
						},
						{
							find: /^@spacedrive\/tokens\/css$/,
							replacement: `${spaceui}/tokens/src/css/base.css`,
						},
						{
							find: /^@spacedrive\/tokens$/,
							replacement: `${spaceui}/tokens`,
						},
						{
							find: /^@spacedrive\/primitives$/,
							replacement: `${spaceui}/primitives/src/index.ts`,
						},
					]
				: []),
			{
				find: "@sd/interface",
				replacement: path.resolve(__dirname, "../../packages/interface/src"),
			},
			{
				find: "@sd/ts-client",
				replacement: path.resolve(__dirname, "../../packages/ts-client/src"),
			},
			{
				find: "openapi-fetch",
				replacement: path.resolve(
					__dirname,
					"../../packages/interface/node_modules/openapi-fetch/dist/index.mjs",
				),
			},
		],
	},
	server: {
		port: 3000,
		fs: {
			allow: [
				path.resolve(__dirname, "../../.."),
				...(hasSpaceui ? [spaceui] : []),
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
		exclude: ["@spacedrive/primitives", "@spacedrive/tokens"],
	},
	build: {
		outDir: "dist",
		emptyOutDir: true,
		sourcemap: true,
	},
});
