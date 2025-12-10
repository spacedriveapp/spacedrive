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
} from "@phosphor-icons/react";
import type { File } from "@sd/ts-client";
import { useContextMenu } from "../../../hooks/useContextMenu";
import { useJobDispatch } from "../../../hooks/useJobDispatch";
import { useLibraryMutation } from "../../../context";
import { usePlatform } from "../../../platform";
import { getContentKind } from "../utils";
import { useExplorer } from "../context";

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
	const { setCurrentPath, currentPath } = useExplorer();
	const platform = usePlatform();
	const copyFiles = useLibraryMutation("files.copy");
	const deleteFiles = useLibraryMutation("files.delete");
	const { runJob } = useJobDispatch();

	// Get the files to operate on (multi-select or just this file)
	const getTargetFiles = () => {
		if (selected && selectedFiles.length > 0) {
			return selectedFiles;
		}
		return [file];
	};

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
						setCurrentPath(file.sd_path);
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
								console.error(
									"Failed to reveal file:",
									err,
								);
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
				onClick: async () => {
					const targets = getTargetFiles();
					const sdPaths = targets.map((f) => f.sd_path);

					console.log(
						"Copying files:",
						targets.map((f) => f.name),
					);

					// Store the file paths for paste
					window.__SPACEDRIVE__ = window.__SPACEDRIVE__ || {};
					window.__SPACEDRIVE__.clipboard = {
						operation: "copy",
						files: sdPaths,
						sourcePath: currentPath,
					};

					console.log(
						`Copied ${sdPaths.length} files to clipboard`,
					);
				},
				keybind: "⌘C",
			},
			{
				icon: Copy,
				label: "Paste",
				onClick: async () => {
					const clipboard = window.__SPACEDRIVE__?.clipboard;
					if (!clipboard || !clipboard.files || !currentPath) {
						console.log("Nothing to paste or no destination");
						return;
					}

					console.log(
						`Pasting ${clipboard.files.length} files to:`,
						currentPath,
					);

					try {
						console.log("Paste params:", {
							sources: clipboard.files,
							destination: currentPath,
						});

						const result = await copyFiles.mutateAsync({
							sources: { paths: clipboard.files },
							destination: currentPath,
							overwrite: false,
							verify_checksum: false,
							preserve_timestamps: true,
							move_files: false,
							copy_method: "Auto" as const,
						});

						console.log("Paste operation result:", result);
						console.log("Result type:", typeof result, result);

						// Check if it's a confirmation request
						if (
							result &&
							typeof result === "object" &&
							"NeedsConfirmation" in result
						) {
							console.log(
								"Action needs confirmation:",
								result,
							);
							alert(
								"File conflict detected - confirmation UI not implemented yet",
							);
						} else if (
							result &&
							typeof result === "object" &&
							"job_id" in result
						) {
							console.log(
								"Job started with ID:",
								result.job_id,
							);
						}
					} catch (err) {
						console.error("Failed to paste:", err);
						alert(`Failed to paste: ${err}`);
					}
				},
				keybind: "⌘V",
				condition: () => {
					const clipboard = window.__SPACEDRIVE__?.clipboard;
					return (
						!!clipboard &&
						!!clipboard.files &&
						clipboard.files.length > 0
					);
				},
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
			},
		],
	});
}
