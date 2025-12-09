import { memo } from "react";
import clsx from "clsx";
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
	MagicWand,
	TextAa,
	Crop,
	FileVideo,
} from "@phosphor-icons/react";
import type { File } from "@sd/ts-client";
import { File as FileComponent } from "../../File";
import { useExplorer } from "../../context";
import { useSelection } from "../../SelectionContext";
import { useContextMenu } from "../../../../hooks/useContextMenu";
import { useJobDispatch } from "../../../../hooks/useJobDispatch";
import { useLibraryMutation } from "../../../../context";
import { usePlatform } from "../../../../platform";
import { formatBytes, getContentKind } from "../../utils";
import { TagDot } from "../../../Tags";
import { useDraggable, useDroppable } from "@dnd-kit/core";

interface FileCardProps {
	file: File;
	fileIndex: number;
	allFiles: File[];
	selected: boolean;
	focused: boolean;
	selectedFiles: File[];
	selectFile: (
		file: File,
		files: File[],
		multi?: boolean,
		range?: boolean,
	) => void;
}

export const FileCard = memo(
	function FileCard({
		file,
		fileIndex,
		allFiles,
		selected,
		focused,
		selectedFiles,
		selectFile,
	}: FileCardProps) {
		const { setCurrentPath, viewSettings, currentPath } = useExplorer();
		const { gridSize, showFileSize } = viewSettings;
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

		const contextMenu = useContextMenu({
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
							label: "Extract Text from All",
							onClick: async () => {
								await runJob("ocr", {
									file_ids: selectedFiles.map((f) => f.id),
								});
							},
						},
						{
							icon: FilmStrip,
							label: "Generate Thumbstrips (Videos)",
							onClick: async () => {
								const videos = selectedFiles.filter(
									(f) => getContentKind(f) === "video",
								);
								if (videos.length > 0) {
									await runJob("thumbstrip", {
										file_ids: videos.map((f) => f.id),
									});
								}
							},
							condition: () =>
								selectedFiles.some(
									(f) => getContentKind(f) === "video",
								),
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
									permanent: false, // Move to trash, not permanent delete
									recursive: true, // Allow deleting non-empty directories
								});
								console.log(
									"Delete operation started:",
									result,
								);
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

		const handleClick = (e: React.MouseEvent) => {
			const multi = e.metaKey || e.ctrlKey;
			const range = e.shiftKey;
			selectFile(file, allFiles, multi, range);
		};

		const handleDoubleClick = () => {
			if (file.kind === "Directory") {
				setCurrentPath(file.sd_path);
			}
		};

		const handleContextMenu = async (e: React.MouseEvent) => {
			e.preventDefault();
			e.stopPropagation();

			if (!selected) {
				selectFile(file, allFiles, false, false);
			}

			await contextMenu.show(e);
		};

		const {
			attributes,
			listeners,
			setNodeRef: setDragNodeRef,
			isDragging: dndIsDragging,
		} = useDraggable({
			id: file.id,
			data: {
				type: "explorer-file",
				sdPath: file.sd_path,
				name: file.name,
				file: file,
				gridSize: gridSize,
				selectedFiles: selected && selectedFiles.length > 0 ? selectedFiles : undefined,
			},
		});

		// Make folders droppable
		const isFolder = file.kind === "Directory";
		const { setNodeRef: setDropNodeRef, isOver: isDropOver } = useDroppable({
			id: `folder-drop-${file.id}`,
			disabled: !isFolder,
			data: {
				action: "move-into",
				targetType: "folder",
				targetId: file.id,
				targetPath: file.sd_path,
			},
		});

		// Combine refs for folders that are both draggable and droppable
		const setNodeRef = (node: HTMLElement | null) => {
			setDragNodeRef(node);
			if (isFolder) setDropNodeRef(node);
		};

		const thumbSize = Math.max(gridSize * 0.6, 60);

		return (
			<div
				ref={setNodeRef}
				{...listeners}
				{...attributes}
				data-file-id={file.id}
				className="relative"
			>
				{/* Drop indicator for folders */}
				{isFolder && isDropOver && (
					<div className="absolute inset-0 rounded-lg ring-2 ring-accent ring-inset pointer-events-none z-10" />
				)}
				<FileComponent
					file={file}
					selected={selected}
					onClick={handleClick}
					onDoubleClick={handleDoubleClick}
					onContextMenu={handleContextMenu}
					layout="column"
					className={clsx(
						"flex flex-col items-center gap-2 p-1 rounded-lg transition-all",
						focused && !selected && "ring-2 ring-accent/50",
						dndIsDragging && "opacity-50",
						isFolder && isDropOver && "bg-accent/10",
					)}
				>
					<div
						className={clsx(
							"rounded-lg p-2",
							selected ? "bg-app-box" : "bg-transparent",
						)}
					>
						<FileComponent.Thumb file={file} size={thumbSize} />
					</div>
					<div className="w-full flex flex-col items-center">
						<div
							className={clsx(
								"text-sm truncate px-2 py-0.5 rounded-md inline-block max-w-full",
								selected ? "bg-accent text-white" : "text-ink",
							)}
						>
							{file.name}
						</div>
						{showFileSize && file.size > 0 && (
							<div className="text-xs text-ink-dull mt-0.5">
								{formatBytes(file.size)}
							</div>
						)}

						{/* Tag Indicators */}
						{file.tags && file.tags.length > 0 && (
							<div
								className="flex items-center gap-1 mt-1"
								title={file.tags
									.map((t) => t.canonical_name)
									.join(", ")}
							>
								{file.tags.slice(0, 3).map((tag) => (
									<TagDot
										key={tag.id}
										color={tag.color || "#3B82F6"}
										tooltip={tag.canonical_name}
									/>
								))}
								{file.tags.length > 3 && (
									<span className="text-[10px] text-ink-faint font-medium">
										+{file.tags.length - 3}
									</span>
								)}
							</div>
						)}
					</div>
				</FileComponent>
			</div>
		);
	},
	(prev, next) => {
		// Custom comparison - rerender if file object, selection, or focus changed
		// Ignore selectedFiles and selectFile function reference changes
		if (prev.file !== next.file) return false; // File object reference changed
		if (prev.selected !== next.selected) return false; // Selection state changed
		if (prev.focused !== next.focused) return false; // Focus state changed
		if (prev.fileIndex !== next.fileIndex) return false; // Index changed
		// Ignore: allFiles, selectedFiles, selectFile (passed through to handlers)
		return true; // Props are equal, skip rerender
	},
);
