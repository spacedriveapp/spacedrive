import type { File } from "@sd/ts-client";
import clsx from "clsx";
import { memo } from "react";
import { File as FileComponent } from "../../File";
import { useFileContextMenu } from "../../hooks/useFileContextMenu";
import { useSelection } from "../../SelectionContext";

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
    range?: boolean
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
      className={clsx(
        "group relative h-full w-full cursor-pointer overflow-hidden outline-none transition-all focus:outline-none",
        selected && "ring-2 ring-accent ring-inset",
        focused && !selected && "ring-2 ring-accent/50 ring-inset"
      )}
      data-file-id={file.id}
      onClick={handleClick}
      onContextMenu={handleContextMenu}
      tabIndex={-1}
    >
      <FileComponent.Thumb
        className="h-full w-full"
        file={file}
        frameClassName="w-full h-full object-cover"
        iconScale={0.5}
        size={size}
        squareMode={true}
      />

      {/* Selection overlay */}
      {selected && (
        <div className="pointer-events-none absolute inset-0 bg-accent/10" />
      )}

      {/* Video duration badge */}
      {file.video_media_data?.duration_seconds && (
        <div className="absolute right-1 bottom-1 rounded bg-black/80 px-1.5 py-0.5 font-medium text-[10px] text-white tabular-nums backdrop-blur-sm">
          {formatDuration(file.video_media_data.duration_seconds)}
        </div>
      )}

      {/* Hover overlay with file name */}
      <div className="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/70 to-transparent px-2 py-1.5 opacity-0 transition-opacity group-hover:opacity-100">
        <div className="truncate font-medium text-white text-xs">
          {file.name}
          {file.extension && `.${file.extension}`}
        </div>
      </div>

      {/* Selection checkbox (top-left corner, always visible when selected) */}
      {selected && (
        <div className="absolute top-1 left-1 flex h-5 w-5 items-center justify-center rounded-full bg-accent">
          <svg
            className="h-3 w-3 text-white"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              d="M5 13l4 4L19 7"
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={3}
            />
          </svg>
        </div>
      )}
    </div>
  );
});
