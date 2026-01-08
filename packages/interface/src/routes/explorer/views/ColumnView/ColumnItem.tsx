import type { File } from "@sd/ts-client";
import clsx from "clsx";
import { memo, useCallback } from "react";
import { File as FileComponent } from "../../File";
import { useDraggableFile } from "../../hooks/useDraggableFile";

interface ColumnItemProps {
  file: File;
  selected: boolean;
  focused: boolean;
  onClick: (multi: boolean, range: boolean) => void;
  onDoubleClick?: () => void;
  onContextMenu?: (e: React.MouseEvent) => void;
}

export const ColumnItem = memo(
  function ColumnItem({
    file,
    selected,
    focused,
    onClick,
    onDoubleClick,
    onContextMenu,
  }: ColumnItemProps) {
    const handleClick = useCallback(
      (e: React.MouseEvent) => {
        const multi = e.metaKey || e.ctrlKey;
        const range = e.shiftKey;
        onClick(multi, range);
      },
      [onClick]
    );

    const handleDoubleClick = useCallback(() => {
      if (onDoubleClick) {
        onDoubleClick();
      }
    }, [onDoubleClick]);

    const { attributes, listeners, setNodeRef, isDragging } = useDraggableFile({
      file,
    });

    return (
      <div
        ref={setNodeRef}
        {...listeners}
        {...attributes}
        className="outline-none focus:outline-none"
        tabIndex={-1}
      >
        <FileComponent
          className={clsx(
            "mx-2 flex cursor-default items-center gap-2 rounded-md px-3 py-1.5 transition-none",
            selected && !isDragging ? "bg-accent text-white" : "text-ink",
            focused && !selected && "ring-2 ring-accent/50",
            isDragging && "opacity-40"
          )}
          data-file-id={file.id}
          file={file}
          layout="row"
          onClick={handleClick}
          onContextMenu={onContextMenu}
          onDoubleClick={handleDoubleClick}
          selected={selected && !isDragging}
        >
          <div className="[&_*]:!rounded-[3px] flex-shrink-0">
            <FileComponent.Thumb file={file} size={20} />
          </div>
          <span className="flex-1 truncate text-sm">
            {file.name}
            {file.extension && `.${file.extension}`}
          </span>
          {file.kind === "Directory" && (
            <svg
              className="size-3 text-ink-dull"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                d="M9 5l7 7-7 7"
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
              />
            </svg>
          )}
        </FileComponent>
      </div>
    );
  },
  (prev, next) => {
    // Only re-render if selection state, focus, or file changed
    if (prev.selected !== next.selected) return false;
    if (prev.focused !== next.focused) return false;
    if (prev.file !== next.file) return false;
    // Ignore onClick, onDoubleClick, onContextMenu function reference changes
    return true;
  }
);
