import type {
	ExtensionModelsInput,
	ExtensionModelsOutput,
	File,
	GetFilesByTagInput,
	GetFilesByTagOutput,
	JsonValue,
	RunExtensionJobOutput,
	SearchTagsInput,
	SearchTagsOutput,
	Tag,
} from "@sd/ts-client";
import { WIRE_METHODS } from "@sd/ts-client";
import { useEffect, useRef, useState } from "react";
import { useParams } from "react-router-dom";
import { useServer } from "../../contexts/ServerContext";
import {
	useCoreQuery,
	useLibraryMutation,
	useSpacedriveClient,
} from "../../contexts/SpacedriveContext";

/**
 * What a view bundle's `mount` receives. Reads and job runs are scoped to
 * the extension that declared the view, so a bundle names its own models
 * and jobs without its id.
 */
export interface ViewContext {
	extensionId: string;
	libraryId: string | null;
	/** The rows of one of the extension's models, newest first. */
	readModels: (model: string, limit?: number) => Promise<unknown[]>;
	/** Start one of the extension's jobs with a starting state. */
	runJob: (job: string, state?: JsonValue) => Promise<RunExtensionJobOutput>;
	/** Every tag in the library, the person's own included. */
	listTags: () => Promise<Tag[]>;
	/** The files carrying a tag or one of its children. */
	filesByTag: (tagId: string) => Promise<File[]>;
	buildSidecarUrl: (
		contentUuid: string,
		kind: string,
		variant: string,
		format: string,
		version?: number,
	) => string | null;
}

type Mount = (el: HTMLElement, ctx: ViewContext) => void | (() => void);

/**
 * A library page an extension declares in `ui_manifest.json` `views`. The
 * bundle is imported from the daemon's `/extension/<id>/<bundle>` route and
 * mounted into a plain element, the way a file viewer is.
 */
export function ExtensionView() {
	const { extensionId = "", viewId = "" } = useParams();
	const client = useSpacedriveClient();
	const { buildSidecarUrl, buildExtensionBundleUrl, libraryId } = useServer();
	const { data } = useCoreQuery({ type: "extensions.list", input: {} });
	const view = data?.extensions
		.find((e) => e.id === extensionId)
		?.views.find((v) => v.id === viewId);
	const url = view ? buildExtensionBundleUrl(extensionId, view.bundle) : null;
	const hostRef = useRef<HTMLDivElement>(null);
	const [error, setError] = useState<string | null>(null);
	const runJob = useLibraryMutation("extensions.run_job");
	// The bundle is mounted once per view; a ref keeps its context on the
	// latest mutation without remounting it.
	const runJobRef = useRef(runJob.mutateAsync);
	runJobRef.current = runJob.mutateAsync;

	useEffect(() => {
		const el = hostRef.current;
		if (!el || !url) return;
		let cancelled = false;
		let unmount: (() => void) | void;
		setError(null);
		const ctx: ViewContext = {
			extensionId,
			libraryId,
			readModels: async (model, limit) => {
				const output = await client.execute<ExtensionModelsInput, ExtensionModelsOutput>(
					WIRE_METHODS.libraryQueries["extensions.models"],
					{ extension_id: extensionId, model, limit: limit ?? null },
				);
				return output.rows;
			},
			runJob: (job, state) =>
				runJobRef.current({ job: `${extensionId}:${job}`, state: state ?? null }),
			listTags: async () => {
				const output = await client.execute<SearchTagsInput, SearchTagsOutput>(
					WIRE_METHODS.libraryQueries["tags.search"],
					{ query: "", limit: null },
				);
				return output.tags;
			},
			filesByTag: async (tagId) => {
				const output = await client.execute<GetFilesByTagInput, GetFilesByTagOutput>(
					WIRE_METHODS.libraryQueries["files.by_tag"],
					{ tag_id: tagId, include_children: true },
				);
				return output.files;
			},
			buildSidecarUrl,
		};
		import(/* @vite-ignore */ url)
			.then((module: { mount?: unknown }) => {
				if (cancelled) return;
				if (typeof module.mount !== "function") {
					throw new Error("the module exports no mount(el, ctx) function");
				}
				unmount = (module.mount as Mount)(el, ctx);
			})
			.catch((reason: unknown) => {
				if (cancelled) return;
				console.warn(`[ExtensionView] ${extensionId}:${viewId} could not be mounted`, reason);
				setError(String(reason));
			});
		return () => {
			cancelled = true;
			unmount?.();
			el.replaceChildren();
		};
		// The context closes over the client and server, which remount the
		// app when they change.
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [url, extensionId, viewId, libraryId]);

	if (data && !view) {
		return (
			<div className="text-ink-dull flex h-full items-center justify-center">
				This extension view is not loaded.
			</div>
		);
	}
	if (error) {
		return (
			<div className="text-ink-dull flex h-full items-center justify-center">
				{error}
			</div>
		);
	}
	return (
		<div
			ref={hostRef}
			data-extension-view={`${extensionId}:${viewId}`}
			className="text-ink h-full w-full"
		/>
	);
}
