import type { File } from "@sd/ts-client";
import { useEffect, useRef, useState } from "react";
import { useServer } from "../../contexts/ServerContext";
import { useOriginalUrl } from "./useOriginalUrl";
import { useHotThumb } from "../../routes/explorer/hooks/useHotThumb";
import type { BuiltinRenderer, ContentRendererProps, Renderer } from "./renderers";

/**
 * What a viewer bundle's `mount` receives. A plain object and a plain DOM
 * element, so the bundle carries no copy of the app's React and does not
 * depend on its version.
 */
export interface BundleContext {
	file: File;
	/** The file's original bytes, once the daemon has a URL for them. */
	originalUrl: string | null;
	/** The largest thumbnail sidecar, or the hot-tier tile before one exists. */
	thumbnailUrl: string | null;
	buildSidecarUrl: (
		contentUuid: string,
		kind: string,
		variant: string,
		format: string,
		version?: number,
	) => string | null;
}

/** The module shape a bundle exports. */
interface ViewerModule {
	mount?: unknown;
}

type Mount = (el: HTMLElement, ctx: BundleContext) => void | (() => void);

interface BundleRendererProps extends ContentRendererProps {
	extensionId: string;
	viewer: string;
	bundle: string;
	fallback: BuiltinRenderer;
	fallbackRenderer: Renderer;
}

/**
 * Hosts an extension's viewer bundle for one file. The module is imported
 * from the daemon's `/extension/<id>/<bundle>` route and its `mount` is
 * called with the file's context; the function it returns runs when the
 * file or the bundle changes and on unmount, and the host element is
 * cleared after it. A bundle that cannot be fetched,
 * has no `mount`, or throws while mounting is logged once and replaced by
 * the parent kind's renderer for that file.
 */
export function BundleRenderer(props: BundleRendererProps) {
	const { file, extensionId, viewer, bundle, fallback, fallbackRenderer } = props;
	const { buildSidecarUrl, buildExtensionBundleUrl } = useServer();
	const originalUrl = useOriginalUrl(file);
	const hostRef = useRef<HTMLDivElement>(null);
	const url = buildExtensionBundleUrl(extensionId, bundle);
	const fileKey = file.content_identity?.uuid ?? file.id;
	const [failedFor, setFailedFor] = useState<string | null>(null);
	const attempt = `${url}\u0000${fileKey}`;

	const thumbnail = (file.sidecars ?? [])
		.filter((s) => s.kind === "thumb")
		.sort(
			(a, b) =>
				parseInt(b.variant.replace(/\D/g, "") || "0") -
				parseInt(a.variant.replace(/\D/g, "") || "0"),
		)[0];
	const sidecarThumbnailUrl =
		thumbnail && file.content_identity?.uuid
			? buildSidecarUrl(
					file.content_identity.uuid,
					thumbnail.kind,
					thumbnail.variant,
					thumbnail.format,
					thumbnail.version,
				)
			: null;
	const hotThumb = useHotThumb(file.sd_path, sidecarThumbnailUrl === null);
	const thumbnailUrl = sidecarThumbnailUrl ?? hotThumb.url;

	useEffect(() => {
		const el = hostRef.current;
		if (!el || !url || failedFor === attempt) {
			return;
		}
		let cancelled = false;
		let unmount: (() => void) | void;
		const fail = (reason: unknown) => {
			if (cancelled) return;
			console.warn(
				`[BundleRenderer] viewer ${viewer} of ${extensionId} (${bundle}) could not be mounted; falling back to the ${fallback} renderer`,
				reason,
			);
			setFailedFor(attempt);
		};
		import(/* @vite-ignore */ url)
			.then((module: ViewerModule) => {
				if (cancelled) return;
				if (typeof module.mount !== "function") {
					fail(new Error("the module exports no mount(el, ctx) function"));
					return;
				}
				try {
					unmount = (module.mount as Mount)(el, {
						file,
						originalUrl,
						thumbnailUrl,
						buildSidecarUrl,
					});
				} catch (error) {
					fail(error);
				}
			})
			.catch(fail);
		return () => {
			cancelled = true;
			unmount?.();
			unmount = undefined;
			// A viewer that returns no remover must not leave its nodes for
			// the next mount to stack on.
			el.replaceChildren();
		};
		// The context is rebuilt when the file or its byte URL changes; the
		// sidecar URL builder only changes with the server, which remounts
		// everything anyway.
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [url, attempt, failedFor, originalUrl, thumbnailUrl]);

	if (!url || failedFor === attempt) {
		const Fallback = fallbackRenderer;
		return <Fallback {...props} />;
	}

	return (
		<div
			ref={hostRef}
			data-extension-viewer={`${extensionId}:${viewer}`}
			className="w-full h-full text-ink"
		/>
	);
}
