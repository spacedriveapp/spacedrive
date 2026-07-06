import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "path";
import fs from "fs";

const repoRoot = path.resolve(__dirname, "../..");

function bunPackageSrc(pkg: string): string | null {
	const bunDir = path.join(repoRoot, "node_modules/.bun");
	if (!fs.existsSync(bunDir)) return null;
	for (const entry of fs.readdirSync(bunDir)) {
		if (entry.startsWith(`${pkg}@`)) {
			const src = path.join(bunDir, entry, "node_modules", pkg, "src/index.ts");
			if (fs.existsSync(src)) return src;
		}
	}
	return null;
}

const styleToJs = bunPackageSrc("style-to-js");
const styleToObject = bunPackageSrc("style-to-object");

// Pre-bundle common CJS deps from the markdown/unified stack (Bun hoists to .bun/).
function bunPackageMain(pkg: string, file = "index.js"): string | null {
	const bunDir = path.join(repoRoot, "node_modules/.bun");
	if (!fs.existsSync(bunDir)) return null;
	for (const entry of fs.readdirSync(bunDir)) {
		if (entry.startsWith(`${pkg}@`)) {
			const main = path.join(bunDir, entry, "node_modules", pkg, file);
			if (fs.existsSync(main)) return main;
		}
	}
	return null;
}

const cjsInteropPackages = [
	"extend",
	"debug",
	"style-to-js",
	"style-to-object",
	"ms",
	"devlop",
	"unist-util-visit",
	"unist-util-is",
];
const cjsInteropIncludes = cjsInteropPackages
	.map((pkg) => bunPackageMain(pkg))
	.filter((p): p is string => p !== null);

const spaceui = path.resolve(__dirname, "../../../spaceui/packages");
const hasSpaceui = fs.existsSync(spaceui);
const spacebot = path.resolve(__dirname, "../../../spacebot/packages");
const hasSpacebot = fs.existsSync(spacebot);

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
							find: /^@spacedrive\/ai$/,
							replacement: `${spaceui}/ai/src/index.ts`,
						},
						{
							find: /^@spacedrive\/primitives$/,
							replacement: `${spaceui}/primitives/src/index.ts`,
						},
					]
				: []),
			...(hasSpacebot
				? [
						{
							find: /^@spacebot\/api-client$/,
							replacement: `${spacebot}/api-client/src`,
						},
					]
				: [
						{
							find: /^@spacebot\/api-client$/,
							replacement: path.resolve(
								__dirname,
								"./stubs/spacebot-api-client.ts",
							),
						},
					]),
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
			...(styleToJs
				? [{ find: /^style-to-js$/, replacement: styleToJs }]
				: []),
			...(styleToObject
				? [{ find: /^style-to-object$/, replacement: styleToObject }]
				: []),
			{
				find: /^debug$/,
				replacement: path.resolve(__dirname, "./stubs/debug.ts"),
			},
			{
				find: /^extend$/,
				replacement: path.resolve(__dirname, "./stubs/extend.ts"),
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
			"/rpc": {
				target: "http://localhost:8080",
				changeOrigin: true,
			},
			"/events": {
				target: "http://localhost:8080",
				changeOrigin: true,
			},
		},
	},
	optimizeDeps: {
		include: cjsInteropIncludes,
		needsInterop: cjsInteropPackages,
		exclude: ["@spacedrive/ai", "@spacedrive/primitives", "@spacedrive/tokens"],
	},
	build: {
		outDir: "dist",
		emptyOutDir: true,
		sourcemap: true,
		rollupOptions: {
			external: [
				...(!hasSpacebot ? ["@spacebot/api-client"] : []),
			],
		},
	},
});
