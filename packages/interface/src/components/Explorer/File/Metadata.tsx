import clsx from "clsx";
import type { File } from "@sd/ts-client/generated/types";
import { formatBytes, formatRelativeTime } from "../utils";

interface MetadataProps {
  file: File;
  show?: Array<"size" | "modified" | "kind">;
  className?: string;
}

export function Metadata({
  file,
  show = ["size"],
  className,
}: MetadataProps) {
  return (
    <div className={clsx("flex gap-2 text-xs text-ink-dull", className)}>
      {show.includes("size") && file.size > 0 && (
        <span>{formatBytes(file.size)}</span>
      )}
      {show.includes("modified") && (
        <span>{formatRelativeTime(file.modified_at)}</span>
      )}
      {show.includes("kind") && (
        <span>{file.extension || "Folder"}</span>
      )}
    </div>
  );
}
