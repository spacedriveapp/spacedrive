import { useCallback } from "react";
import type { File } from "@sd/ts-client";
import { useLibraryMutation } from "../../../contexts/SpacedriveContext";

/**
 * Shared hook for delete file operations.
 * Used by both useExplorerKeyboard (DEL key) and useFileContextMenu.
 */
export function useDeleteFiles() {
	const mutation = useLibraryMutation("files.delete");

	const deleteFiles = useCallback(
		async (files: File[], permanent: boolean) => {
			if (files.length === 0) return false;
			if (files.some((f) => !f.sd_path)) return false;
			if (mutation.isPending) return false;

			const label = permanent ? "Permanently delete" : "Delete";
			const suffix = permanent ? " This cannot be undone." : "";
			const message =
				files.length > 1
					? `${label} ${files.length} items?${suffix}`
					: `${label} "${files[0].name}"?${suffix}`;

			if (!confirm(message)) return false;

			try {
				await mutation.mutateAsync({
					targets: { paths: files.map((f) => f.sd_path) },
					permanent,
					recursive: true,
				});
				return true;
			} catch (err) {
				console.error("Failed to delete:", err);
				alert(`Failed to delete: ${err}`);
				return false;
			}
		},
		[mutation],
	);

	return { deleteFiles, isPending: mutation.isPending };
}
