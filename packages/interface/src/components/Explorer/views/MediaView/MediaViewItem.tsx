import clsx from "clsx";
import { memo } from "react";
import { Eye, Copy, Trash, MagnifyingGlass } from "@phosphor-icons/react";
import type { File } from "@sd/ts-client/generated/types";
import { File as FileComponent } from "../../File";
import { useExplorer } from "../../context";
import { useContextMenu } from "../../../../hooks/useContextMenu";
import { useLibraryMutation } from "../../../../context";
import { usePlatform } from "../../../../platform";

function formatDuration(seconds: number): string {
	const mins = Math.floor(seconds / 60);
	const secs = Math.floor(seconds % 60);
	return `${mins}:${String(secs).padStart(2, '0')}`;
}

interface MediaViewItemProps {
	file: File;
	selected: boolean;
	focused: boolean;
	onSelect: (file: File, multi?: boolean, range?: boolean) => void;
	size: number;
}

export const MediaViewItem = memo(function MediaViewItem({
	file,
	selected,
	focused,
	onSelect,
	size,
}: MediaViewItemProps) {
	const { selectedFiles, currentPath } = useExplorer();
	const platform = usePlatform();
	const copyFiles = useLibraryMutation("files.copy");
	const deleteFiles = useLibraryMutation("files.delete");

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
				},
				keybind: "Space",
			},
			{
				icon: MagnifyingGlass,
				label: "Show in Finder",
				onClick: async () => {
					if ("Physical" in file.sd_path) {
						const physicalPath = file.sd_path.Physical.path;
						if (platform.revealFile) {
							try {
								await platform.revealFile(physicalPath);
							} catch (err) {
								console.error("Failed to reveal file:", err);
								alert(`Failed to reveal file: ${err}`);
							}
						}
					}
				},
				keybind: "⌘⇧R",
				condition: () => "Physical" in file.sd_path && !!platform.revealFile,
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

					window.__SPACEDRIVE__ = window.__SPACEDRIVE__ || {};
					window.__SPACEDRIVE__.clipboard = {
						operation: "copy",
						files: sdPaths,
						sourcePath: currentPath,
					};

					console.log(`Copied ${sdPaths.length} files to clipboard`);
				},
				keybind: "⌘C",
			},
			{
				icon: Copy,
				label: "Paste",
				onClick: async () => {
					const clipboard = window.__SPACEDRIVE__?.clipboard;
					if (!clipboard || !clipboard.files || !currentPath) {
						return;
					}

					try {
						await copyFiles.mutateAsync({
							sources: { paths: clipboard.files },
							destination: currentPath,
							overwrite: false,
							verify_checksum: false,
							preserve_timestamps: true,
							move_files: false,
							copy_method: "Auto" as const,
						});
					} catch (err) {
						console.error("Failed to paste:", err);
					}
				},
				keybind: "⌘V",
				condition: () => {
					const clipboard = window.__SPACEDRIVE__?.clipboard;
					return !!clipboard && !!clipboard.files && clipboard.files.length > 0;
				},
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
						try {
							await deleteFiles.mutateAsync({
								targets: { paths: targets.map((f) => f.sd_path) },
								permanent: false,
								recursive: true,
							});
						} catch (err) {
							console.error("Failed to delete:", err);
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
		onSelect(file, multi, range);
	};

	const handleContextMenu = async (e: React.MouseEvent) => {
		e.preventDefault();
		e.stopPropagation();

		if (!selected) {
			onSelect(file, false, false);
		}

		await contextMenu.show(e);
	};

	return (
		<div
			className={clsx(
				"relative overflow-hidden cursor-pointer transition-all w-full h-full group",
				selected && "ring-2 ring-accent ring-inset",
				focused && !selected && "ring-2 ring-accent/50 ring-inset",
			)}
			onClick={handleClick}
			onContextMenu={handleContextMenu}
		>
			<FileComponent.Thumb
				file={file}
				size={size}
				className="w-full h-full"
				frameClassName="w-full h-full object-cover"
				iconScale={0.5}
			/>

			{/* Selection overlay */}
			{selected && (
				<div className="absolute inset-0 bg-accent/10 pointer-events-none" />
			)}

			{/* Video duration badge */}
			{file.video_media_data?.duration_seconds && (
				<div className="absolute bottom-1 right-1 px-1.5 py-0.5 rounded bg-black/80 text-white text-[10px] font-medium backdrop-blur-sm tabular-nums">
					{formatDuration(file.video_media_data.duration_seconds)}
				</div>
			)}

			{/* Hover overlay with file name */}
			<div className="absolute inset-x-0 bottom-0 px-2 py-1.5 bg-gradient-to-t from-black/70 to-transparent opacity-0 group-hover:opacity-100 transition-opacity">
				<div className="text-white text-xs font-medium truncate">
					{file.name}
				</div>
			</div>

			{/* Selection checkbox (top-left corner, always visible when selected) */}
			{selected && (
				<div className="absolute top-1 left-1 w-5 h-5 rounded-full bg-accent flex items-center justify-center">
					<svg
						className="w-3 h-3 text-white"
						fill="none"
						viewBox="0 0 24 24"
						stroke="currentColor"
					>
						<path
							strokeLinecap="round"
							strokeLinejoin="round"
							strokeWidth={3}
							d="M5 13l4 4L19 7"
						/>
					</svg>
				</div>
			)}
		</div>
	);
});
