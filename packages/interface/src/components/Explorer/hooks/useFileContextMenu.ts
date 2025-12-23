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

interface UseFileContextMenuProps {
	file: File;
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
	const { runJob } = useJobDispatch();
	const clipboard = useClipboard();
	const openFileOperation = useFileOperationDialog();

	// Get the files to operate on (multi-select or just this file)
	// Filters out virtual files (they're display-only, not real filesystem entries)
	const getTargetFiles = () => {
		const targets =
			selected && selectedFiles.length > 0 ? selectedFiles : [file];
		// Filter out virtual files - they cannot be copied/moved/deleted
		return targets.filter((f) => !isVirtualFile(f));
	};

	// Check if any selected files are virtual (to disable certain operations)
	const hasVirtualFiles = selected
		? selectedFiles.some((f) => isVirtualFile(f))
		: isVirtualFile(file);

	return useContextMenu({
		items: [
			{
				icon: Eye,
				label: "Quick Look",
				onClick: () => {
					console.log("Quick Look:", file.name);
					// TODO: Implement quick look
				},
				keybind: "Space",
			},
			{
				icon: FolderOpen,
				label: "Open",
				onClick: () => {
					if (file.kind === "Directory") {
						navigateToPath(file.sd_path);
					} else {
						console.log("Open file:", file.name);
						// TODO: Implement file opening
					}
				},
				keybind: "⌘O",
				condition: () =>
					file.kind === "Directory" || file.kind === "File",
			},
			{
				icon: MagnifyingGlass,
				label: "Show in Finder",
				onClick: async () => {
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
					"Physical" in file.sd_path && !!platform.revealFile,
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
						console.log("Nothing to paste or no destination");
						return;
					}

					const operation =
						clipboard.operation === "cut" ? "move" : "copy";

					openFileOperation({
						operation,
						sources: clipboard.files,
						destination: currentPath,
						onComplete: () => {
							// Clear clipboard after cut operation completes
							if (clipboard.operation === "cut") {
								clipboard.clearClipboard();
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
				condition: () => getContentKind(file) === "image",
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
						condition: () => !file.image_media_data?.blurhash,
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
				condition: () => getContentKind(file) === "video",
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
						condition: () => !file.video_media_data?.blurhash,
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
				condition: () => getContentKind(file) === "audio",
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
							: `Delete "${file.name}"?`;

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
