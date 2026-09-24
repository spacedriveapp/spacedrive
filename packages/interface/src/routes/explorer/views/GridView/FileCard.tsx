import { memo } from "react";
import clsx from "clsx";
import type { File, SdPath } from "@sd/ts-client";
import { File as FileComponent } from "../../File";
import { useExplorer } from "../../context";
import { useSelection } from "../../SelectionContext";
import { formatBytes } from "../../utils";
import { TagDot, tagColor } from "../../../../components/Tags";
import { useDndContext, useDroppable } from "@dnd-kit/core";
import { useLibraryValidate } from "../../../../contexts/SpacedriveContext";
import { useOverlaidChange } from "../../hooks/usePlanPreview";
import { PlanBadge, planRowClass } from "../../components/PlanMark";
import { useFileContextMenu } from "../../hooks/useFileContextMenu";
import { useDraggableFile } from "../../hooks/useDraggableFile";
import { isVirtualFile } from '@sd/ts-client';
import { VolumeSizeBar } from "../../components/VolumeSizeBar";
import { InlineNameEdit } from "../../components/InlineNameEdit";
import { useOpenWith } from "../../../../hooks/useOpenWith";

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
		focused: _focused,
		selectedFiles,
		selectFile,
	}: FileCardProps) {
		const { viewSettings, navigateToPath } = useExplorer();
		const { gridSize, showFileSize } = viewSettings;
		const { renamingFileId, saveRename, cancelRename } = useSelection();

		const isRenaming = renamingFileId === file.id;

		const contextMenu = useFileContextMenu({
			file,
			selectedFiles,
			selected,
		});

		// Set up file opening for non-directory files
		const physicalPath =
			file.kind === "File" && "Physical" in file.sd_path
				? [(file.sd_path as any).Physical.path]
				: [];
		const { openWithDefault } = useOpenWith(physicalPath);

		const handleClick = (e: React.MouseEvent) => {
			const multi = e.metaKey || e.ctrlKey;
			const range = e.shiftKey;
			selectFile(file, allFiles, multi, range);
		};

		const handleDoubleClick = async () => {
			// Virtual files (sources, volumes, devices) always navigate to their sd_path
			if (isVirtualFile(file) && file.sd_path) {
				navigateToPath(file.sd_path);
				return;
			}

			// Regular directories navigate normally
			if (file.kind === "Directory") {
				navigateToPath(file.sd_path);
				return;
			}

			// Open regular files with default application
			if (file.kind === "File" && "Physical" in file.sd_path) {
				const physicalPath = (file.sd_path as any).Physical.path;
				await openWithDefault(physicalPath);
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
		} = useDraggableFile({
			file,
			selectedFiles: selected && selectedFiles.length > 0 ? selectedFiles : undefined,
			gridSize,
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

		// While a drag hovers this folder, copy validation says whether the
		// drop could run, so the target badges an error before the drop.
		const { active: dragActive } = useDndContext();
		const hovering = isFolder && isDropOver && dragActive?.data.current?.type === "explorer-file";
		const hoverSources: SdPath[] = hovering
			? (dragActive?.data.current?.selectedFiles as File[] | undefined)?.map((f) => f.sd_path) ??
				[dragActive?.data.current?.sdPath]
			: [];
		const hoverValidation = useLibraryValidate(
			{
				type: "files.copy",
				input: {
					sources: { paths: hoverSources },
					destination: file.sd_path,
					overwrite: false,
					verify_checksum: false,
					preserve_timestamps: true,
					move_files: false,
					copy_method: "Auto",
					on_conflict: "Skip",
				},
			},
			{ enabled: hovering && hoverSources.length > 0, staleTime: 5_000 },
		);
		const hoverFinding = hovering
			? hoverValidation.data?.findings.find((finding) => finding.severity === "error") ??
				hoverValidation.data?.findings.find((finding) => finding.severity === "warning")
			: undefined;

		// The change a browsed plan makes to this row, if the explorer is in
		// preview mode.
		const planned = useOverlaidChange(file);

		const thumbSize = Math.max(gridSize * 0.6, 60);

		// Check if this is a virtual volume file
		const isVolume =
			isVirtualFile(file) &&
			(file as any)._virtual?.type === "volume" &&
			(file as any)._virtual?.data;

		// Extract volume data
		const volumeData = isVolume ? (file as any)._virtual.data : null;
		const hasVolumeCapacity =
			volumeData?.total_capacity != null &&
			volumeData?.available_space != null &&
			volumeData.total_capacity > 0;

		return (
			<div
				ref={setNodeRef}
				{...listeners}
				{...attributes}
				data-file-id={file.id}
				data-index={fileIndex}
				data-selectable="true"
				tabIndex={-1}
				className="relative outline-none focus:outline-none"
			>
				{/* Drop indicator for folders */}
				{isFolder && isDropOver && (
					<div
						className={clsx(
							"absolute inset-0 rounded-lg ring-2 ring-inset pointer-events-none z-10",
							hoverFinding?.severity === "error" ? "ring-red-500" : "ring-accent",
						)}
					/>
				)}
				{hoverFinding && (
					<div
						className={clsx(
							"absolute left-1 right-1 top-1 z-20 rounded px-1.5 py-0.5 text-[10px] font-medium leading-tight pointer-events-none truncate",
							hoverFinding.severity === "error"
								? "bg-red-500 text-white"
								: "bg-amber-500 text-white",
						)}
						title={hoverFinding.message}
					>
						{hoverFinding.message}
					</div>
				)}
				{planned && (
					<PlanBadge change={planned} className="absolute right-1 top-1 z-20" />
				)}
				<FileComponent
					file={file}
					selected={selected && !dndIsDragging}
					onClick={handleClick}
					onDoubleClick={handleDoubleClick}
					onContextMenu={handleContextMenu}
					layout="column"
					className={clsx(
						"flex flex-col items-center gap-2 p-1 rounded-lg transition-all",
						dndIsDragging && "opacity-40",
						isFolder && isDropOver && "bg-accent/10",
						planRowClass(planned),
						(planned?.type === "create" ||
							planned?.type === "create_directory" ||
							planned?.type === "move") &&
							"outline-dashed outline-1 outline-accent/50",
					)}
				>
					<div
						className={clsx(
							"rounded-lg p-2",
							selected && !dndIsDragging ? "bg-app-box" : "bg-transparent",
						)}
					>
						<FileComponent.Thumb file={file} size={thumbSize} />
					</div>
					<div className="w-full flex flex-col items-center">
						{isRenaming ? (
							<InlineNameEdit
								file={file}
								onSave={saveRename}
								onCancel={cancelRename}
								className="max-w-full"
							/>
						) : (
							<div
								className={clsx(
									"text-sm truncate px-2 py-0.5 rounded-md inline-block max-w-full",
									selected && !dndIsDragging ? "bg-accent text-white" : "text-ink",
								)}
							>
								{file.name}{file.extension && `.${file.extension}`}
							</div>
						)}

						{/* Volume size bar */}
						{showFileSize && hasVolumeCapacity && (
							<VolumeSizeBar
								totalBytes={Number(volumeData.total_capacity)}
								availableBytes={Number(volumeData.available_space)}
								className="mt-1.5"
							/>
						)}

						{/* Regular file size */}
						{showFileSize && !hasVolumeCapacity && file.size > 0 && (
							<div className="text-xs text-ink-dull mt-0.5">
								{formatBytes(file.size)}
							</div>
						)}

						{/* Tag Indicators */}
						{file.tags && file.tags.length > 0 && (
							<div
								className="flex items-center gap-1 mt-1"
								title={file.tags
									.map((t) => t.name)
									.join(", ")}
							>
								{file.tags.slice(0, 3).map((tag) => (
									<TagDot
										key={tag.id}
										color={tagColor(tag)}
										tooltip={tag.name}
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