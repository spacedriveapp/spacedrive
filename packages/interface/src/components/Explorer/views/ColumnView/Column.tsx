import { useRef, memo, useCallback } from "react";
import { useVirtualizer, type VirtualItem } from "@tanstack/react-virtual";
import clsx from "clsx";
import type { File, SdPath } from "@sd/ts-client";
import { useNormalizedQuery } from "../../../../context";
import { ColumnItem } from "./ColumnItem";
import { useExplorer } from "../../context";
import { useFileContextMenu } from "../../hooks/useFileContextMenu";
import { useSelection } from "../../SelectionContext";

/**
 * Memoized wrapper for ColumnItem to prevent re-renders when selection changes elsewhere.
 * Only re-renders when this specific item's `selected` state changes.
 */
const ColumnItemWrapper = memo(
	function ColumnItemWrapper({
		file,
		files,
		virtualRow,
		selected,
		selectedFiles,
		onSelectFile,
	}: {
		file: File;
		files: File[];
		virtualRow: VirtualItem;
		selected: boolean;
		selectedFiles: File[];
		onSelectFile: (
			file: File,
			files: File[],
			multi?: boolean,
			range?: boolean,
		) => void;
	}) {
		const contextMenu = useFileContextMenu({
			file,
			selectedFiles,
			selected,
		});

		const handleClick = useCallback(
			(multi: boolean, range: boolean) => {
				onSelectFile(file, files, multi, range);
			},
			[file, files, onSelectFile],
		);

		const handleContextMenu = useCallback(
			async (e: React.MouseEvent) => {
				e.preventDefault();
				e.stopPropagation();
				if (!selected) {
					onSelectFile(file, files, false, false);
				}
				await contextMenu.show(e);
			},
			[file, files, selected, onSelectFile, contextMenu],
		);

		return (
			<div
				style={{
					position: "absolute",
					top: 0,
					left: 0,
					width: "100%",
					height: `${virtualRow.size}px`,
					transform: `translateY(${virtualRow.start}px)`,
				}}
			>
				<ColumnItem
					file={file}
					selected={selected}
					focused={false}
					onClick={handleClick}
					onContextMenu={handleContextMenu}
				/>
			</div>
		);
	},
	(prev, next) => {
		// Only re-render if selection state or file changed
		if (prev.selected !== next.selected) return false;
		if (prev.file !== next.file) return false;
		if (prev.virtualRow.start !== next.virtualRow.start) return false;
		if (prev.virtualRow.size !== next.virtualRow.size) return false;
		// Ignore: files array, onSelectFile, contextMenu (passed through to handlers)
		return true;
	},
);

interface ColumnProps {
	path: SdPath;
	isSelected: (fileId: string) => boolean;
	selectedFileIds: Set<string>;
	onSelectFile: (
		file: File,
		files: File[],
		multi?: boolean,
		range?: boolean,
	) => void;
	onNavigate: (path: SdPath) => void;
	nextColumnPath?: SdPath;
	columnIndex: number;
	isActive: boolean;
}

export const Column = memo(function Column({
	path,
	isSelected,
	selectedFileIds,
	onSelectFile,
	onNavigate,
	nextColumnPath,
	columnIndex,
	isActive,
}: ColumnProps) {
	const parentRef = useRef<HTMLDivElement>(null);
	const { viewSettings, sortBy } = useExplorer();
	const { selectedFiles } = useSelection();

	const directoryQuery = useNormalizedQuery({
		wireMethod: "query:files.directory_listing",
		input: {
			path: path,
			limit: null,
			include_hidden: false,
			sort_by: sortBy as any,
			folders_first: viewSettings.foldersFirst,
		},
		resourceType: "file",
		pathScope: path,
		// includeDescendants defaults to false for exact directory matching
	});

	const files = directoryQuery.data?.files || [];

	const rowVirtualizer = useVirtualizer({
		count: files.length,
		getScrollElement: () => parentRef.current,
		estimateSize: () => 32,
		overscan: 10,
	});


	if (directoryQuery.isLoading) {
		return (
			<div
				className="shrink-0 border-r border-app-line flex items-center justify-center"
				style={{ width: `${viewSettings.columnWidth}px` }}
			>
				<div className="text-sm text-ink-dull">Loading...</div>
			</div>
		);
	}

	return (
		<div
			ref={parentRef}
			className={clsx(
				"shrink-0 border-r border-app-line overflow-auto",
				isActive && "bg-app-box/30",
			)}
			style={{ width: `${viewSettings.columnWidth}px` }}
		>
			<div
				style={{
					height: `${rowVirtualizer.getTotalSize()}px`,
					width: "100%",
					position: "relative",
				}}
			>
				{rowVirtualizer.getVirtualItems().map((virtualRow) => {
					const file = files[virtualRow.index];

					// Check if this file is selected using O(1) lookup
					const fileIsSelected = isSelected(file.id);

					// Check if this file is part of the navigation path
					const isInPath =
						nextColumnPath &&
						file.sd_path.Physical &&
						nextColumnPath.Physical
							? file.sd_path.Physical.path ===
									nextColumnPath.Physical.path &&
								file.sd_path.Physical.device_slug ===
									nextColumnPath.Physical.device_slug
							: false;

					return (
						<ColumnItemWrapper
							key={virtualRow.key}
							file={file}
							files={files}
							virtualRow={virtualRow}
							selected={fileIsSelected || isInPath}
							selectedFiles={selectedFiles}
							onSelectFile={onSelectFile}
						/>
					);
				})}
			</div>
		</div>
	);
});
