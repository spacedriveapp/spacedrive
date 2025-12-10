import { memo, useCallback } from "react";
import { flexRender, type Row } from "@tanstack/react-table";
import clsx from "clsx";

import type { File } from "@sd/ts-client";

import { File as FileComponent } from "../../File";
import { useExplorer } from "../../context";
import { TagPill } from "../../../Tags";
import { ROW_HEIGHT, TABLE_PADDING_X } from "./useTable";

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
		const { setCurrentPath } = useExplorer();

		const handleClick = useCallback(
			(e: React.MouseEvent) => {
				const multi = e.metaKey || e.ctrlKey;
				const range = e.shiftKey;
				selectFile(file, files, multi, range);
			},
			[file, files, selectFile],
		);

		const handleDoubleClick = useCallback(() => {
			if (file.kind === "Directory") {
				setCurrentPath(file.sd_path);
			}
		}, [file, setCurrentPath]);

		const cells = row.getVisibleCells();

		return (
			<div
				ref={measureRef}
				data-index={index}
				data-file-id={file.id}
				className="relative"
				style={{ height: ROW_HEIGHT }}
				onClick={handleClick}
				onDoubleClick={handleDoubleClick}
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
	return (
		<div className="flex min-w-0 flex-1 items-center gap-2">
			{/* File icon */}
			<div className="flex-shrink-0">
				<FileComponent.Thumb file={file} size={20} />
			</div>

			{/* File name */}
			<span className="truncate text-sm text-ink">{file.name}</span>

			{/* Tags (inline, compact) */}
			{file.tags && file.tags.length > 0 && (
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
