import type { ContentKind, File, ListExtensionsOutput, ExtensionKindInfo } from "@sd/ts-client";
import { getContentKind } from "@sd/ts-client";
import type { ReactNode } from "react";
import type { VideoControlsCallbacks, VideoControlsState } from "./VideoControls";

/**
 * The preview renderer registry. Built-in renderers register here by name at
 * import; an extension kind names one of them through `preview.renderer` or a
 * viewer bundle through `preview.viewer`, and `resolvePreview` turns a file
 * plus the kinds `extensions.list` reports into the one thing to mount. A
 * stored kind name whose extension is not loaded matches nothing and falls
 * through to the parent kind's renderer, so unloading an extension needs no
 * client-side bookkeeping.
 */

export interface ContentRendererProps {
	file: File;
	onZoomChange?: (isZoomed: boolean) => void;
	onVideoControlsStateChange?: (state: VideoControlsState) => void;
	onShowVideoControlsChange?: (show: boolean) => void;
	getVideoCallbacks?: (callbacks: VideoControlsCallbacks) => void;
}

export type Renderer = (props: ContentRendererProps) => ReactNode;

/** The names a manifest's `preview.renderer` may use; `BUILTIN_RENDERERS` in core. */
export type BuiltinRenderer =
	| "image"
	| "video"
	| "audio"
	| "mesh"
	| "document"
	| "text"
	| "default";

const BUILTIN_RENDERERS: readonly string[] = [
	"image",
	"video",
	"audio",
	"mesh",
	"document",
	"text",
	"default",
];

const renderers = new Map<BuiltinRenderer, Renderer>();

export function registerRenderer(name: BuiltinRenderer, renderer: Renderer) {
	renderers.set(name, renderer);
}

export function getRenderer(name: BuiltinRenderer): Renderer | undefined {
	return renderers.get(name) ?? renderers.get("default");
}

/** The built-in renderer that previews a built-in kind. */
export function rendererForKind(kind: ContentKind): BuiltinRenderer {
	switch (kind) {
		case "image":
			return "image";
		case "video":
			return "video";
		case "audio":
			return "audio";
		case "mesh":
			return "mesh";
		case "document":
		case "book":
		case "spreadsheet":
		case "presentation":
			return "document";
		case "text":
		case "code":
		case "config":
			return "text";
		default:
			return "default";
	}
}

/** A kind of a loaded extension, with its viewer's bundle resolved. */
export interface ExtensionKind extends ExtensionKindInfo {
	extensionId: string;
	/** Path inside the extension directory when `preview.viewer` names a declared viewer. */
	bundle: string | null;
}

export type ExtensionKinds = ReadonlyMap<string, ExtensionKind>;

export const NO_EXTENSION_KINDS: ExtensionKinds = new Map();

/** Every loaded extension's kinds keyed by kind id. */
export function indexExtensionKinds(
	list: ListExtensionsOutput | undefined,
): ExtensionKinds {
	const kinds = new Map<string, ExtensionKind>();
	for (const extension of list?.extensions ?? []) {
		for (const kind of extension.kinds) {
			const viewer =
				kind.preview && "viewer" in kind.preview
					? kind.preview.viewer
					: null;
			const bundle =
				viewer === null
					? null
					: (extension.viewers.find((v) => v.id === viewer)?.bundle ??
						null);
			kinds.set(kind.id, { ...kind, extensionId: extension.id, bundle });
		}
	}
	return kinds;
}

export type Preview =
	| { type: "renderer"; name: BuiltinRenderer }
	| {
			type: "bundle";
			extensionId: string;
			viewer: string;
			bundle: string;
			/** The renderer to use when the bundle fails to load or mount. */
			fallback: BuiltinRenderer;
	  };

/**
 * What previews a file: the stored kind name resolved against the loaded
 * extensions first, then the parent kind against the built-ins.
 */
export function resolvePreview(file: File, kinds: ExtensionKinds): Preview {
	const parent = rendererForKind(getContentKind(file));
	const kind = file.content_kind_name
		? kinds.get(file.content_kind_name)
		: undefined;
	if (!kind) {
		return { type: "renderer", name: parent };
	}
	const own = rendererForKind(kind.parent);
	if (!kind.preview) {
		return { type: "renderer", name: own };
	}
	if ("renderer" in kind.preview) {
		const name = kind.preview.renderer;
		return {
			type: "renderer",
			name: BUILTIN_RENDERERS.includes(name) ? (name as BuiltinRenderer) : own,
		};
	}
	if (kind.bundle === null) {
		return { type: "renderer", name: own };
	}
	return {
		type: "bundle",
		extensionId: kind.extensionId,
		viewer: kind.preview.viewer,
		bundle: kind.bundle,
		fallback: own,
	};
}

/**
 * What to call a file's kind: the loaded extension's display name, the
 * stored kind id when its extension is gone, or the built-in kind.
 */
export function kindLabel(file: File, kinds: ExtensionKinds): string {
	const name = file.content_kind_name;
	if (!name) {
		return getContentKind(file);
	}
	return kinds.get(name)?.display_name ?? name;
}
