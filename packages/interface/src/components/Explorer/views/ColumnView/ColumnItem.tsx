import clsx from "clsx";
import type { File } from "@sd/ts-client";
import { useDraggable } from "@dnd-kit/core";
import { File as FileComponent } from "../../File";

interface ColumnItemProps {
  file: File;
  selected: boolean;
  focused: boolean;
  onClick: (multi: boolean, range: boolean) => void;
  onDoubleClick?: () => void;
  onContextMenu?: (e: React.MouseEvent) => void;
}

export function ColumnItem({
  file,
  selected,
  focused,
  onClick,
  onDoubleClick,
  onContextMenu,
}: ColumnItemProps) {
  const handleClick = (e: React.MouseEvent) => {
    const multi = e.metaKey || e.ctrlKey;
    const range = e.shiftKey;
    onClick(multi, range);
  };

  const handleDoubleClick = () => {
    if (onDoubleClick) {
      onDoubleClick();
    }
  };

  const { attributes, listeners, setNodeRef, isDragging } = useDraggable({
    id: file.id,
    data: {
      type: "explorer-file",
      sdPath: file.sd_path,
      name: file.name,
      file: file,
    },
  });

  return (
    <div ref={setNodeRef} {...listeners} {...attributes}>
      <FileComponent
        file={file}
        selected={selected}
        onClick={handleClick}
        onDoubleClick={handleDoubleClick}
        onContextMenu={onContextMenu}
        layout="row"
        data-file-id={file.id}
        className={clsx(
          "flex items-center gap-2 px-3 py-1.5 mx-2 rounded-md cursor-default transition-none",
          selected
            ? "bg-accent text-white"
            : "text-ink",
          focused && !selected && "ring-2 ring-accent/50",
          isDragging && "opacity-50"
        )}
      >
        <div className="[&_*]:!rounded-[3px] flex-shrink-0">
          <FileComponent.Thumb file={file} size={20} />
        </div>
        <span className="text-sm truncate flex-1">{file.name}</span>
        {file.kind === "Directory" && (
          <svg
            className="size-3 text-ink-dull"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M9 5l7 7-7 7"
            />
          </svg>
        )}
      </FileComponent>
    </div>
  );
}
