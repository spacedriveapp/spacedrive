import { useCallback } from "react";
import { toast } from "@spacedrive/primitives";
import type { File, FileLinkInput, SdPath, Validation } from "@sd/ts-client";
import { RefusedError, WIRE_METHODS } from "@sd/ts-client";
import { useLibraryMutation, useSpacedriveClient } from "../../../contexts/SpacedriveContext";
import { pathString } from "../../../components/modals/PreflightPanel";

/**
 * Make a symlink beside a file, named after it: `files.link` after its
 * validation, with nothing to choose and so no dialog.
 */
export function useMakeLink() {
	const client = useSpacedriveClient();
	const link = useLibraryMutation("files.link");

	const makeLink = useCallback(
		async (file: File) => {
			if (!("Physical" in file.sd_path)) return false;
			const whole = pathString(file.sd_path);
			const cut = whole.lastIndexOf("/");
			const directory = whole.slice(0, cut);
			const at: SdPath = {
				Physical: {
					device_slug: file.sd_path.Physical.device_slug,
					path: `${directory}/${file.name} link${file.extension ? `.${file.extension}` : ""}`,
				},
			};
			const input: FileLinkInput = { at, target: file.sd_path, kind: "symlink" };
			try {
				const validation = await client.execute<FileLinkInput, Validation>(
					WIRE_METHODS.libraryValidates["files.link"],
					input,
				);
				const errors = validation.findings.filter((finding) => finding.severity === "error");
				if (errors.length > 0) {
					toast.error(errors.map((finding) => finding.message).join("; "));
					return false;
				}
				await link.mutateAsync(input);
				toast.success(`Linked ${file.name}`);
				return true;
			} catch (error) {
				toast.error(
					error instanceof RefusedError
						? error.validation.findings.map((finding) => finding.message).join("; ")
						: `Could not link: ${error instanceof Error ? error.message : String(error)}`,
				);
				return false;
			}
		},
		[client, link],
	);

	return { makeLink, isPending: link.isPending };
}
