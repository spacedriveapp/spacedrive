import { describe, expect, test } from "bun:test";
import type { File, ListExtensionsOutput } from "@sd/ts-client";
import {
	indexExtensionKinds,
	kindLabel,
	rendererForKind,
	resolvePreview,
	NO_EXTENSION_KINDS,
} from "./renderers";

const list: ListExtensionsOutput = {
	supported: true,
	conflicts: [],
	extensions: [
		{
			id: "com.spacedrive.photos",
			name: "Photos",
			version: "1.0.0",
			jobs: [],
			kinds: [
				{
					id: "com.spacedrive.photos:raw",
					name: "raw",
					display_name: "RAW photo",
					parent: "image",
					extensions: ["cr2", "dng"],
					preview: { renderer: "image" },
				},
			],
			viewers: [{ id: "photo_viewer", bundle: "ui/photo_viewer.js" }],
		},
		{
			id: "test-extension",
			name: "Test Extension",
			version: "0.1.0",
			jobs: [],
			kinds: [
				{
					id: "test-extension:fake",
					name: "fake",
					display_name: "Fake file",
					parent: "text",
					extensions: ["fake"],
					preview: { viewer: "fake_viewer" },
				},
				{
					id: "test-extension:dangling",
					name: "dangling",
					display_name: "Dangling viewer",
					parent: "document",
					extensions: ["dng2"],
					preview: { viewer: "missing" },
				},
				{
					id: "test-extension:plain",
					name: "plain",
					display_name: "Plain",
					parent: "audio",
					extensions: ["pln"],
					preview: null,
				},
			],
			viewers: [{ id: "fake_viewer", bundle: "ui/fake-viewer.js" }],
		},
	],
};

function file(content_kind: File["content_kind"], content_kind_name: string | null): File {
	return {
		content_kind,
		content_kind_name,
		kind: "File",
		name: "x",
	} as unknown as File;
}

describe("indexExtensionKinds", () => {
	test("keys every loaded kind by id and resolves its viewer bundle", () => {
		const kinds = indexExtensionKinds(list);
		expect([...kinds.keys()]).toEqual([
			"com.spacedrive.photos:raw",
			"test-extension:fake",
			"test-extension:dangling",
			"test-extension:plain",
		]);
		expect(kinds.get("test-extension:fake")?.bundle).toBe("ui/fake-viewer.js");
		expect(kinds.get("test-extension:fake")?.extensionId).toBe("test-extension");
		expect(kinds.get("test-extension:dangling")?.bundle).toBeNull();
		expect(kinds.get("com.spacedrive.photos:raw")?.bundle).toBeNull();
	});

	test("an absent list is no kinds", () => {
		expect(indexExtensionKinds(undefined).size).toBe(0);
	});
});

describe("resolvePreview", () => {
	const kinds = indexExtensionKinds(list);

	test("a built-in kind uses its renderer", () => {
		expect(resolvePreview(file("video", null), kinds)).toEqual({
			type: "renderer",
			name: "video",
		});
		expect(resolvePreview(file("spreadsheet", null), kinds)).toEqual({
			type: "renderer",
			name: "document",
		});
		expect(resolvePreview(file("archive", null), kinds)).toEqual({
			type: "renderer",
			name: "default",
		});
	});

	test("a loaded kind with preview.renderer uses that built-in", () => {
		expect(
			resolvePreview(file("image", "com.spacedrive.photos:raw"), kinds),
		).toEqual({ type: "renderer", name: "image" });
	});

	test("a loaded kind with preview.viewer mounts the bundle with the parent as fallback", () => {
		expect(resolvePreview(file("text", "test-extension:fake"), kinds)).toEqual({
			type: "bundle",
			extensionId: "test-extension",
			viewer: "fake_viewer",
			bundle: "ui/fake-viewer.js",
			fallback: "text",
		});
	});

	test("a viewer the UI manifest does not declare falls back to the parent renderer", () => {
		expect(
			resolvePreview(file("document", "test-extension:dangling"), kinds),
		).toEqual({ type: "renderer", name: "document" });
	});

	test("a kind without a preview uses its parent's renderer", () => {
		expect(resolvePreview(file("audio", "test-extension:plain"), kinds)).toEqual({
			type: "renderer",
			name: "audio",
		});
	});

	test("a stored name whose extension is unloaded falls back to the parent kind", () => {
		expect(
			resolvePreview(file("image", "com.spacedrive.photos:raw"), NO_EXTENSION_KINDS),
		).toEqual({ type: "renderer", name: "image" });
		expect(
			resolvePreview(file("text", "test-extension:fake"), NO_EXTENSION_KINDS),
		).toEqual({ type: "renderer", name: "text" });
	});
});

describe("kindLabel", () => {
	const kinds = indexExtensionKinds(list);

	test("names a loaded kind by its display name and an unloaded one by its id", () => {
		expect(kindLabel(file("image", "com.spacedrive.photos:raw"), kinds)).toBe("RAW photo");
		expect(kindLabel(file("image", "com.spacedrive.photos:raw"), NO_EXTENSION_KINDS)).toBe(
			"com.spacedrive.photos:raw",
		);
		expect(kindLabel(file("image", null), kinds)).toBe("image");
	});
});

describe("rendererForKind", () => {
	test("maps the document and text families to their shared renderers", () => {
		expect(rendererForKind("book")).toBe("document");
		expect(rendererForKind("code")).toBe("text");
		expect(rendererForKind("mesh")).toBe("mesh");
		expect(rendererForKind("unknown")).toBe("default");
	});
});
