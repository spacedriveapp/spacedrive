import clsx from "clsx";
import type { File as FileType } from "@sd/ts-client";
import { Thumb } from "./Thumb";
import { Title } from "./Title";
import { Metadata } from "./Metadata";

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
      data-file-id={dataFileId}
      onClick={onClick}
      onDoubleClick={onDoubleClick}
      onContextMenu={onContextMenu}
      onMouseDown={onMouseDown}
      onMouseMove={onMouseMove}
      onMouseUp={onMouseUp}
      onMouseLeave={onMouseLeave}
      className={clsx(
        "cursor-default transition-colors",
        layout === "column" ? "flex flex-col" : "flex flex-row items-center",
        className
      )}
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
