import { memo, useCallback } from "react";
import { flexRender, type Row } from "@tanstack/react-table";
import clsx from "clsx";

import type { File } from "@sd/ts-client";

import { File as FileComponent } from "../../File";
import { useExplorer } from "../../context";
import { useSelection } from "../../SelectionContext";
import { TagPill } from "../../../Tags";
import { ROW_HEIGHT, TABLE_PADDING_X } from "./useTable";
import { useFileContextMenu } from "../../hooks/useFileContextMenu";
import { isVirtualFile } from "../../utils/virtualFiles";
import { InlineNameEdit } from "../../components/InlineNameEdit";
import { useOpenWith } from "../../../../hooks/useOpenWith";

interface TableRowProps {
	row: Row<File>;
	file: File;
	files: File[];
	index: number;
	isSelected: boolean;
	isFocused: boolean;
	isPreviousSelected: boolean;
	isNextSelected: boolean;
	measureRef: (node: HTMLElement | null) => void;
	selectFile: (
		file: File,
		files: File[],
		multi?: boolean,
		range?: boolean,
	) => void;
}

export const TableRow = memo(
	function TableRow({
		row,
		file,
		files,
		index,
		isSelected,
		isFocused,
		isPreviousSelected,
		isNextSelected,
		measureRef,
		selectFile,
	}: TableRowProps) {
		const { navigateToPath } = useExplorer();
		const { selectedFiles } = useSelection();

		const contextMenu = useFileContextMenu({
			file,
			selectedFiles,
			selected: isSelected,
		});

		// Set up file opening for non-directory files
		const physicalPath =
			file.kind === "File" && "Physical" in file.sd_path
				? [(file.sd_path as any).Physical.path]
				: [];
		const { openWithDefault } = useOpenWith(physicalPath);

		const handleClick = useCallback(
			(e: React.MouseEvent) => {
				const multi = e.metaKey || e.ctrlKey;
				const range = e.shiftKey;
				selectFile(file, files, multi, range);
			},
			[file, files, selectFile],
		);

		const handleDoubleClick = useCallback(async () => {
			// Virtual files (locations, volumes, devices) always navigate to their sd_path
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
		}, [file, navigateToPath, openWithDefault]);

		const handleContextMenu = useCallback(
			async (e: React.MouseEvent) => {
				e.preventDefault();
				e.stopPropagation();

				if (!isSelected) {
					selectFile(file, files, false, false);
				}

				await contextMenu.show(e);
			},
			[file, files, isSelected, selectFile, contextMenu],
		);

		const cells = row.getVisibleCells();

		return (
			<div
				ref={measureRef}
				data-index={index}
				data-file-id={file.id}
				data-selectable="true"
				tabIndex={-1}
				className="relative outline-none focus:outline-none"
				style={{ height: ROW_HEIGHT }}
				onClick={handleClick}
				onDoubleClick={handleDoubleClick}
				onContextMenu={handleContextMenu}
			>
				{/* Background layer for alternating colors and selection */}
				<div
					className={clsx(
						"absolute inset-0 rounded-md border",
						// Alternating background
						index % 2 === 0 && !isSelected && "bg-app-darkBox/50",
						// Selection styling
						isSelected
							? "border-accent bg-accent/10"
							: "border-transparent",
						// Connect adjacent selected rows
						isSelected &&
							isPreviousSelected &&
							"rounded-t-none border-t-0",
						isSelected &&
							isNextSelected &&
							"rounded-b-none border-b-0",
					)}
					style={{
						left: TABLE_PADDING_X,
						right: TABLE_PADDING_X,
					}}
				>
					{/* Subtle separator between connected selected rows */}
					{isSelected && isPreviousSelected && (
						<div className="absolute inset-x-3 top-0 h-px bg-accent/20" />
					)}
				</div>

				{/* Row content */}
				<div
					className="relative flex h-full items-center"
					style={{
						paddingLeft: TABLE_PADDING_X,
						paddingRight: TABLE_PADDING_X,
					}}
				>
					{cells.map((cell) => {
						const isNameColumn = cell.column.id === "name";

						return (
							<div
								key={cell.id}
								className={clsx(
									"flex h-full items-center px-2 text-sm",
									isNameColumn
										? "min-w-0 flex-1"
										: "text-ink-dull",
								)}
								style={{ width: cell.column.getSize() }}
							>
								{isNameColumn ? (
									<NameCell file={file} />
								) : (
									<span className="truncate">
										{flexRender(
											cell.column.columnDef.cell,
											cell.getContext(),
										)}
									</span>
								)}
							</div>
						);
					})}
				</div>
			</div>
		);
	},
	(prev, next) => {
		// Only re-render if these specific props changed
		if (prev.isSelected !== next.isSelected) return false;
		if (prev.isFocused !== next.isFocused) return false;
		if (prev.isPreviousSelected !== next.isPreviousSelected) return false;
		if (prev.isNextSelected !== next.isNextSelected) return false;
		if (prev.file !== next.file) return false;
		if (prev.index !== next.index) return false;
		// Ignore: row, files, measureRef, selectFile (function references)
		return true;
	},
);

// Name cell with icon and tags
const NameCell = memo(function NameCell({ file }: { file: File }) {
	const { renamingFileId, saveRename, cancelRename } = useSelection();
	const isRenaming = renamingFileId === file.id;

	return (
		<div className="flex min-w-0 flex-1 items-center gap-2">
			{/* File icon */}
			<div className="flex-shrink-0">
				<FileComponent.Thumb file={file} size={20} />
			</div>

			{/* File name or inline edit */}
			{isRenaming ? (
				<InlineNameEdit
					file={file}
					onSave={saveRename}
					onCancel={cancelRename}
					className="flex-1 min-w-0"
				/>
			) : (
				<span className="truncate text-sm text-ink">{file.name}{file.extension && `.${file.extension}`}</span>
			)}

			{/* Tags (inline, compact) - hide when renaming */}
			{!isRenaming && file.tags && file.tags.length > 0 && (
				<div className="flex flex-shrink-0 items-center gap-1">
					{file.tags.slice(0, 2).map((tag) => (
						<TagPill
							key={tag.id}
							color={tag.color || "#3B82F6"}
							size="xs"
						>
							{tag.canonical_name}
						</TagPill>
					))}
					{file.tags.length > 2 && (
						<span className="text-[10px] text-ink-faint">
							+{file.tags.length - 2}
						</span>
					)}
				</div>
			)}
		</div>
	);
});
