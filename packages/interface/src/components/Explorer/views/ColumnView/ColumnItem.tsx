import clsx from "clsx";
import type { File } from "@sd/ts-client";
import { File as FileComponent } from "../../File";

interface ColumnItemProps {
  file: File;
  selected: boolean;
  focused: boolean;
  onClick: (file: File, multi?: boolean, range?: boolean) => void;
  onContextMenu?: (e: React.MouseEvent) => void;
}

export function ColumnItem({
  file,
  selected,
  focused,
  onClick,
  onContextMenu,
}: ColumnItemProps) {
  const handleClick = (e: React.MouseEvent) => {
    const multi = e.metaKey || e.ctrlKey;
    const range = e.shiftKey;
    onClick(file, multi, range);
  };

  return (
    <FileComponent
      file={file}
      selected={selected}
      onClick={handleClick}
      onContextMenu={onContextMenu}
      layout="row"
      data-file-id={file.id}
      className={clsx(
        "flex items-center gap-2 px-3 py-1.5 mx-2 rounded-md transition-colors cursor-default",
        selected
          ? "bg-accent text-white"
          : "hover:bg-app-hover text-ink",
        focused && !selected && "ring-2 ring-accent/50"
      )}
    >
      <FileComponent.Thumb file={file} size={20} />
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
  );
}
