import clsx from "clsx";
import { memo } from "react";
import type { File } from "@sd/ts-client";
import { File as FileComponent } from "../../File";
import { useSelection } from "../../SelectionContext";
import { useFileContextMenu } from "../../hooks/useFileContextMenu";

function formatDuration(seconds: number): string {
	const mins = Math.floor(seconds / 60);
	const secs = Math.floor(seconds % 60);
	return `${mins}:${String(secs).padStart(2, "0")}`;
}

interface MediaViewItemProps {
	file: File;
	allFiles: File[];
	selected: boolean;
	focused: boolean;
	onSelect: (
		file: File,
		files: File[],
		multi?: boolean,
		range?: boolean,
	) => void;
	size: number;
}

export const MediaViewItem = memo(function MediaViewItem({
	file,
	allFiles,
	selected,
	focused,
	onSelect,
	size,
}: MediaViewItemProps) {
	const { selectedFiles } = useSelection();

	const contextMenu = useFileContextMenu({
		file,
		selectedFiles,
		selected,
	});

	const handleClick = (e: React.MouseEvent) => {
		const multi = e.metaKey || e.ctrlKey;
		const range = e.shiftKey;
		onSelect(file, allFiles, multi, range);
	};

	const handleContextMenu = async (e: React.MouseEvent) => {
		e.preventDefault();
		e.stopPropagation();

		if (!selected) {
			onSelect(file, allFiles, false, false);
		}

		await contextMenu.show(e);
	};

	return (
		<div
			data-file-id={file.id}
			tabIndex={-1}
			className={clsx(
				"relative overflow-hidden cursor-pointer transition-all w-full h-full group outline-none focus:outline-none",
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
				squareMode={true}
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
					{file.name}{file.extension && `.${file.extension}`}
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
