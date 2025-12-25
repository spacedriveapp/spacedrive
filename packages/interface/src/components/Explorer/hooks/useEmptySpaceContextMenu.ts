import { FolderPlus, Copy } from "@phosphor-icons/react";
import { useContextMenu } from "../../../hooks/useContextMenu";
import { useLibraryMutation } from "../../../context";
import { useExplorer } from "../context";
import { useClipboard } from "../../../hooks/useClipboard";
import { useFileOperationDialog } from "../../FileOperationModal";

export function useEmptySpaceContextMenu() {
	const { currentPath } = useExplorer();
	const createFolder = useLibraryMutation("files.createFolder");
	const clipboard = useClipboard();
	const openFileOperation = useFileOperationDialog();

	return useContextMenu({
		items: [
			{
				icon: FolderPlus,
				label: "New Folder",
				onClick: async () => {
					if (!currentPath) return;
					try {
						const result = await createFolder.mutateAsync({
							parent: currentPath,
							name: "Untitled Folder",
							items: [],
						});
						console.log("Created folder:", result);
					} catch (err) {
						console.error("Failed to create folder:", err);
						alert(`Failed to create folder: ${err}`);
					}
				},
				condition: () => !!currentPath,
			},
			{
				icon: Copy,
				label: "Paste",
				onClick: () => {
					if (!clipboard.hasClipboard() || !currentPath) {
						console.log("[Clipboard] Nothing to paste or no destination");
						return;
					}

					const operation =
						clipboard.operation === "cut" ? "move" : "copy";

					console.groupCollapsed(
						`[Clipboard] Pasting ${clipboard.files.length} file${clipboard.files.length === 1 ? "" : "s"} (${operation})`,
					);
					console.log("Operation:", operation);
					console.log("Destination:", currentPath);
					console.log("Source files (SdPath objects):");
					clipboard.files.forEach((file, index) => {
						console.log(`  [${index}]:`, JSON.stringify(file, null, 2));
					});
					console.groupEnd();

					openFileOperation({
						operation,
						sources: clipboard.files,
						destination: currentPath,
						onComplete: () => {
							if (clipboard.operation === "cut") {
								console.log(
									"[Clipboard] Operation completed, clearing clipboard",
								);
								clipboard.clearClipboard();
							} else {
								console.log("[Clipboard] Copy operation completed");
							}
						},
					});
				},
				keybindId: "explorer.paste",
				condition: () => clipboard.hasClipboard(),
			},
		],
	});
}
