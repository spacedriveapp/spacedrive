import { env } from "~/env";

/**
 * Accessor for the browser's `window` object, so that `window` is
 * not access during SSG.
 */
export function getWindow(): (Window & typeof globalThis) | null {
	return typeof window !== 'undefined' ? window : null;
}

export function toTitleCase(str: string) {
	return str
		.toLowerCase()
		.replace(/(?:^|[\s-/])\w/g, function (match) {
			return match.toUpperCase();
		})
		.replaceAll('-', ' ');
}

// https://github.com/mrdoob/three.js/blob/7fa8637df3edcf21a516e1ebbb9b327136457baa/src/renderers/WebGLRenderer.js#L266
const webGLCtxNames = ['webgl2', 'webgl', 'experimental-webgl'];
export function detectWebGLContext() {
	const { WebGLRenderingContext, WebGL2RenderingContext } = window;
	if (WebGLRenderingContext == null) return false;

	const canvas = window.document.createElement('canvas');
	return webGLCtxNames
		.map((ctxName) => {
			try {
				return canvas.getContext(ctxName);
			} catch {
				return null;
			}
		})
		.some(
			(ctx) =>
				ctx != null &&
				(ctx instanceof WebGLRenderingContext ||
					(WebGL2RenderingContext != null && ctx instanceof WebGL2RenderingContext)) &&
				ctx.getParameter(ctx.VERSION) != null
		);
}

export async function getLatestSpacedriveVersion() {
	try {
		const r = await fetch(`https://api.github.com/repos/${env.GITHUB_ORG}/${env.GITHUB_REPO}/releases/latest`);
		const data = await r.json();
		return data.name;
	} catch {
		return "Alpha";
	}
}

