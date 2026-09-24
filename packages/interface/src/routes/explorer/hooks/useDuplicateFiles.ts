import { useCallback } from "react";
import { toast } from "@spacedrive/primitives";
import type { File, FileCopyInput, SdPath, Validation } from "@sd/ts-client";
import { RefusedError, WIRE_METHODS } from "@sd/ts-client";
import { useLibraryMutation, useSpacedriveClient } from "../../../contexts/SpacedriveContext";
import { pathString } from "../../../components/modals/PreflightPanel";

/**
 * Duplicate files in place: `files.copy` into each file's own directory,
 * keeping both, so the copy is written beside the original under a numbered
 * name. There is nothing to choose, so no dialog: the input is validated
 * first, a refusal is shown, and the same input is dispatched.
 */
export function useDuplicateFiles() {
	const client = useSpacedriveClient();
	const copy = useLibraryMutation("files.copy");

	const duplicate = useCallback(
		async (files: File[]) => {
			const byParent = new Map<string, { parent: SdPath; sources: SdPath[] }>();
			for (const file of files) {
				if (!("Physical" in file.sd_path)) continue;
				const parent = parentOf(file.sd_path);
				if (!parent) continue;
				const key = pathString(parent);
				const group = byParent.get(key) ?? { parent, sources: [] };
				group.sources.push(file.sd_path);
				byParent.set(key, group);
			}
			if (byParent.size === 0) return false;

			let dispatched = 0;
			for (const { parent, sources } of byParent.values()) {
				const input: FileCopyInput = {
					sources: { paths: sources },
					destination: parent,
					overwrite: false,
					verify_checksum: false,
					preserve_timestamps: true,
					move_files: false,
					copy_method: "Auto",
					on_conflict: "AutoModifyName",
				};
				try {
					const validation = await client.execute<FileCopyInput, Validation>(
						WIRE_METHODS.libraryValidates["files.copy"],
						input,
					);
					const errors = validation.findings.filter(
						(finding) => finding.severity === "error",
					);
					if (errors.length > 0) {
						toast.error(errors.map((finding) => finding.message).join("; "));
						continue;
					}
					await copy.mutateAsync(input);
					dispatched += sources.length;
				} catch (error) {
					const message =
						error instanceof RefusedError
							? error.validation.findings.map((finding) => finding.message).join("; ")
							: error instanceof Error
								? error.message
								: String(error);
					toast.error(`Could not duplicate: ${message}`);
				}
			}
			if (dispatched > 0) {
				toast.success(`Duplicating ${dispatched} ${dispatched === 1 ? "item" : "items"}`);
			}
			return dispatched > 0;
		},
		[client, copy],
	);

	return { duplicate, isPending: copy.isPending };
}

/** The directory a physical path sits in. */
function parentOf(path: SdPath): SdPath | null {
	if (!("Physical" in path)) return null;
	const whole = path.Physical.path;
	const cut = whole.lastIndexOf("/");
	if (cut <= 0) return null;
	return { Physical: { device_slug: path.Physical.device_slug, path: whole.slice(0, cut) } };
}
