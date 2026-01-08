import type { File as FileType } from "@sd/ts-client";
import clsx from "clsx";
import { Metadata } from "./Metadata";
import { Thumb } from "./Thumb";
import { Title } from "./Title";

interface FileProps {
  file: FileType;
  selected?: boolean;
  onClick?: (e: React.MouseEvent) => void;
  onDoubleClick?: (e: React.MouseEvent) => void;
  onContextMenu?: (e: React.MouseEvent) => void;
  onMouseDown?: (e: React.MouseEvent) => void;
  onMouseMove?: (e: React.MouseEvent) => void;
  onMouseUp?: (e: React.MouseEvent) => void;
  onMouseLeave?: (e: React.MouseEvent) => void;
  layout?: "column" | "row";
  children?: React.ReactNode;
  className?: string;
  "data-file-id"?: string;
}

function FileComponent({
  file,
  selected,
  onClick,
  onDoubleClick,
  onContextMenu,
  onMouseDown,
  onMouseMove,
  onMouseUp,
  onMouseLeave,
  layout = "column",
  children,
  className,
  "data-file-id": dataFileId,
}: FileProps) {
  return (
    <div
      className={clsx(
        "cursor-default outline-none transition-colors focus:outline-none",
        layout === "column" ? "flex flex-col" : "flex flex-row items-center",
        className
      )}
      data-file-id={dataFileId}
      onClick={onClick}
      onContextMenu={onContextMenu}
      onDoubleClick={onDoubleClick}
      onMouseDown={onMouseDown}
      onMouseLeave={onMouseLeave}
      onMouseMove={onMouseMove}
      onMouseUp={onMouseUp}
      tabIndex={-1}
    >
      {children}
    </div>
  );
}

export const File = Object.assign(FileComponent, {
  Thumb,
  Title,
  Metadata,
});
