import {
	Copy,
	Trash,
	Eye,
	FolderOpen,
	MagnifyingGlass,
	Image,
	Video,
	Microphone,
	FileText,
	Stack,
	Sparkle,
	FilmStrip,
	Waveform,
	TextAa,
	Crop,
	FileVideo,
	Scissors,
	Pencil,
	FolderPlus,
	ArrowSquareOut,
} from "@phosphor-icons/react";
import type { File } from "@sd/ts-client";
import { useContextMenu } from "../../../hooks/useContextMenu";
import { useJobDispatch } from "../../../hooks/useJobDispatch";
import { useLibraryMutation } from "../../../context";
import { usePlatform } from "../../../platform";
import { getContentKind } from "../utils";
import { useExplorer } from "../context";
import { isVirtualFile } from "../utils/virtualFiles";
import { useClipboard } from "../../../hooks/useClipboard";
import { useFileOperationDialog } from "../../FileOperationModal";
import { useSelection } from "../SelectionContext";
import { useOpenWith } from "../../../hooks/useOpenWith";

interface UseFileContextMenuProps {
	file?: File | null;
	selectedFiles: File[];
	selected: boolean;
}

export function useFileContextMenu({
	file,
	selectedFiles,
	selected,
}: UseFileContextMenuProps) {
	const { navigateToPath, currentPath } = useExplorer();
	const platform = usePlatform();
	const copyFiles = useLibraryMutation("files.copy");
	const deleteFiles = useLibraryMutation("files.delete");
	const createFolder = useLibraryMutation("files.createFolder");
	const { runJob } = useJobDispatch();
	const clipboard = useClipboard();
	const openFileOperation = useFileOperationDialog();
	const { startRename } = useSelection();

	// Get physical paths for file opening
	const getPhysicalPaths = () => {
		const targets =
			selected && selectedFiles.length > 0 ? selectedFiles : [file];
		return targets
			.filter((f) => f && f.sd_path && "Physical" in f.sd_path)
			.map((f) => (f.sd_path as any).Physical.path);
	};

	const physicalPaths = getPhysicalPaths();
	const { apps, openWithDefault, openWithApp, openMultipleWithApp } =
		useOpenWith(physicalPaths);

	// Get the files to operate on (multi-select or just this file)
	// Filters out virtual files (they're display-only, not real filesystem entries)
	const getTargetFiles = () => {
		const targets =
			selected && selectedFiles.length > 0 ? selectedFiles : [file];
		// Filter out virtual files - they cannot be copied/moved/deleted
		return targets.filter((f) => f && !isVirtualFile(f));
	};

	// Check if any selected files are virtual (to disable certain operations)
	const hasVirtualFiles = selected
		? selectedFiles.some((f) => isVirtualFile(f))
		: file ? isVirtualFile(file) : false;

	return useContextMenu({
		items: [
			{
				icon: Eye,
				label: "Quick Look",
				onClick: () => {
					if (!file) return;
					console.log("Quick Look:", file.name);
					// TODO: Implement quick look
				},
				keybind: "Space",
				condition: () => !!file,
			},
			{
				icon: FolderOpen,
				label: "Open",
				onClick: async () => {
					if (!file) return;
					if (file.kind === "Directory") {
						navigateToPath(file.sd_path);
					} else if ("Physical" in file.sd_path) {
						const physicalPath = (file.sd_path as any).Physical.path;
						await openWithDefault(physicalPath);
					}
				},
				keybind: "⌘O",
				condition: () => !!file && (file.kind === "Directory" || file.kind === "File"),
			},
			{
				type: "submenu",
				icon: ArrowSquareOut,
				label: "Open With",
				condition: () =>
					!!file &&
					file.kind === "File" &&
					"Physical" in file.sd_path &&
					apps.length > 0,
				submenu: apps.map((app) => ({
					label: app.name,
					onClick: async () => {
						if (!file) return;
						if (selected && selectedFiles.length > 1) {
							await openMultipleWithApp(physicalPaths, app.id);
						} else if ("Physical" in file.sd_path) {
							const physicalPath = (file.sd_path as any).Physical
								.path;
							await openWithApp(physicalPath, app.id);
						}
					},
				})),
			},
			{
				icon: MagnifyingGlass,
				label: "Show in Finder",
				onClick: async () => {
					if (!file) return;
					// Extract the physical path from SdPath
					if ("Physical" in file.sd_path) {
						const physicalPath = file.sd_path.Physical.path;
						if (platform.revealFile) {
							try {
								await platform.revealFile(physicalPath);
							} catch (err) {
								console.error("Failed to reveal file:", err);
								alert(`Failed to reveal file: ${err}`);
							}
						} else {
							console.log(
								"revealFile not supported on this platform",
							);
						}
					} else {
						console.log("Cannot reveal non-physical file");
					}
				},
				keybind: "⌘⇧R",
				condition: () =>
					!!file && "Physical" in file.sd_path && !!platform.revealFile,
			},
			{ type: "separator" },
			{
				icon: Pencil,
				label: "Rename",
				onClick: () => {
					if (!file) return;
					startRename(file.id);
				},
				keybindId: "explorer.renameFile",
				condition: () => !!file && selected && selectedFiles.length === 1 && !hasVirtualFiles,
			},
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
				icon: FolderPlus,
				label: "New Folder with Items",
				onClick: async () => {
					if (!currentPath) return;
					const targets = getTargetFiles();
					if (targets.length === 0) return;

					try {
						const result = await createFolder.mutateAsync({
							parent: currentPath,
							name: "New Folder",
							items: targets.map((f) => f.sd_path),
						});
						console.log("Created folder with items:", result);
					} catch (err) {
						console.error("Failed to create folder with items:", err);
						alert(`Failed to create folder: ${err}`);
					}
				},
				condition: () => !!currentPath && selectedFiles.length > 0 && !hasVirtualFiles,
			},
			{ type: "separator" },
			{
				icon: Copy,
				label:
					selected && selectedFiles.length > 1
						? `Copy ${selectedFiles.length} items`
						: "Copy",
				onClick: () => {
					const targets = getTargetFiles();
					if (targets.length === 0) {
						console.warn("Cannot copy virtual files");
						return;
					}
					const sdPaths = targets.map((f) => f.sd_path);
					clipboard.copyFiles(sdPaths, currentPath);
				},
				keybindId: "explorer.copy",
				condition: () => !hasVirtualFiles,
			},
			{
				icon: Scissors,
				label:
					selected && selectedFiles.length > 1
						? `Cut ${selectedFiles.length} items`
						: "Cut",
				onClick: () => {
					const targets = getTargetFiles();
					if (targets.length === 0) {
						console.warn("Cannot cut virtual files");
						return;
					}
					const sdPaths = targets.map((f) => f.sd_path);
					clipboard.cutFiles(sdPaths, currentPath);
				},
				keybindId: "explorer.cut",
				condition: () => !hasVirtualFiles,
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
							// Clear clipboard after cut operation completes
							if (clipboard.operation === "cut") {
								console.log(
									"[Clipboard] Operation completed, clearing clipboard",
								);
								clipboard.clearClipboard();
							} else {
								console.log(
									"[Clipboard] Copy operation completed",
								);
							}
						},
					});
				},
				keybindId: "explorer.paste",
				condition: () => clipboard.hasClipboard(),
			},
			// Media Processing submenu
			{
				type: "submenu",
				icon: Image,
				label: "Image Processing",
				condition: () => !!file && getContentKind(file) === "image",
				submenu: [
					{
						icon: Sparkle,
						label: "Generate Blurhash",
						onClick: async () => {
							const targets = getTargetFiles();
							await runJob("thumbnail", {
								file_ids: targets.map((f) => f.id),
								generate_blurhash: true,
							});
						},
						condition: () => !!file && !file.image_media_data?.blurhash,
					},
					{
						icon: Crop,
						label: "Regenerate Thumbnail",
						onClick: async () => {
							const targets = getTargetFiles();
							await runJob("thumbnail", {
								file_ids: targets.map((f) => f.id),
								force: true,
							});
						},
					},
					{
						icon: TextAa,
						label: "Extract Text (OCR)",
						onClick: async () => {
							const targets = getTargetFiles();
							await runJob("ocr", {
								file_ids: targets.map((f) => f.id),
							});
						},
						keybind: "⌘⇧T",
					},
				],
			},
			{
				type: "submenu",
				icon: Video,
				label: "Video Processing",
				condition: () => !!file && getContentKind(file) === "video",
				submenu: [
					{
						icon: FilmStrip,
						label: "Generate Thumbstrip",
						onClick: async () => {
							const targets = getTargetFiles();
							await runJob("thumbstrip", {
								file_ids: targets.map((f) => f.id),
								frame_count: 10,
							});
						},
						condition: () =>
							!!file &&
							!file.sidecars?.some(
								(s) => s.kind === "thumbstrip",
							),
					},
					{
						icon: Sparkle,
						label: "Generate Blurhash",
						onClick: async () => {
							const targets = getTargetFiles();
							await runJob("thumbnail", {
								file_ids: targets.map((f) => f.id),
								generate_blurhash: true,
							});
						},
						condition: () => !!file && !file.video_media_data?.blurhash,
					},
					{
						icon: Crop,
						label: "Regenerate Thumbnail",
						onClick: async () => {
							const targets = getTargetFiles();
							await runJob("thumbnail", {
								file_ids: targets.map((f) => f.id),
								force: true,
							});
						},
					},
					{
						icon: Waveform,
						label: "Extract Subtitles",
						onClick: async () => {
							const targets = getTargetFiles();
							await runJob("speech_to_text", {
								file_ids: targets.map((f) => f.id),
								output_format: "srt",
							});
						},
					},
					{
						icon: FileVideo,
						label: "Generate Proxy",
						onClick: async () => {
							const targets = getTargetFiles();
							await runJob("proxy", {
								file_ids: targets.map((f) => f.id),
								quality: "720p",
							});
						},
						keybind: "⌘⇧P",
					},
				],
			},
			{
				type: "submenu",
				icon: Microphone,
				label: "Audio Processing",
				condition: () => !!file && getContentKind(file) === "audio",
				submenu: [
					{
						icon: TextAa,
						label: "Transcribe Audio",
						onClick: async () => {
							const targets = getTargetFiles();
							await runJob("speech_to_text", {
								file_ids: targets.map((f) => f.id),
								model: "whisper-base",
							});
						},
						keybind: "⌘⇧T",
					},
				],
			},
			{
				type: "submenu",
				icon: FileText,
				label: "Document Processing",
				condition: () =>
					!!file &&
					file.kind === "File" &&
					["pdf", "doc", "docx"].includes(file.extension || ""),
				submenu: [
					{
						icon: TextAa,
						label: "Extract Text (OCR)",
						onClick: async () => {
							const targets = getTargetFiles();
							await runJob("ocr", {
								file_ids: targets.map((f) => f.id),
							});
						},
						keybind: "⌘⇧T",
					},
					{
						icon: Crop,
						label: "Regenerate Thumbnail",
						onClick: async () => {
							const targets = getTargetFiles();
							await runJob("thumbnail", {
								file_ids: targets.map((f) => f.id),
								force: true,
							});
						},
					},
				],
			},
			// Batch operations submenu
			{
				type: "submenu",
				icon: Stack,
				label: `Process ${selectedFiles.length} Items`,
				condition: () => selected && selectedFiles.length > 1,
				submenu: [
					{
						icon: Crop,
						label: "Regenerate All Thumbnails",
						onClick: async () => {
							await runJob("thumbnail", {
								file_ids: selectedFiles.map((f) => f.id),
								force: true,
							});
						},
					},
					{
						icon: Sparkle,
						label: "Generate Blurhashes",
						onClick: async () => {
							await runJob("thumbnail", {
								file_ids: selectedFiles.map((f) => f.id),
								generate_blurhash: true,
							});
						},
						keybind: "⌘⇧B",
					},
					{
						icon: TextAa,
						label: "Extract Text (OCR)",
						onClick: async () => {
							await runJob("ocr", {
								file_ids: selectedFiles.map((f) => f.id),
							});
						},
					},
				],
			},
			{ type: "separator" },
			{
				icon: Trash,
				label:
					selected && selectedFiles.length > 1
						? `Delete ${selectedFiles.length} items`
						: "Delete",
				onClick: async () => {
					const targets = getTargetFiles();
					if (targets.length === 0) {
						console.warn("Cannot delete virtual files");
						return;
					}
					const message =
						targets.length > 1
							? `Delete ${targets.length} items?`
							: `Delete "${file?.name ?? "this file"}"?`;

					if (confirm(message)) {
						console.log(
							"Deleting files:",
							targets.map((f) => f.name),
						);

						try {
							const result = await deleteFiles.mutateAsync({
								targets: {
									paths: targets.map((f) => f.sd_path),
								},
								permanent: false,
								recursive: true,
							});

							console.log("Delete result:", result);

							// Check if it's a confirmation request
							if (
								result &&
								typeof result === "object" &&
								"NeedsConfirmation" in result
							) {
								console.log(
									"Delete needs confirmation:",
									result,
								);
								alert(
									"Delete confirmation UI not implemented yet",
								);
							} else if (
								result &&
								typeof result === "object" &&
								"job_id" in result
							) {
								console.log(
									"Delete job started:",
									result.job_id,
								);
							}
						} catch (err) {
							console.error("Failed to delete:", err);
							alert(`Failed to delete: ${err}`);
						}
					}
				},
				keybind: "⌘⌫",
				variant: "danger" as const,
				condition: () => !hasVirtualFiles,
			},
		],
	});
}
