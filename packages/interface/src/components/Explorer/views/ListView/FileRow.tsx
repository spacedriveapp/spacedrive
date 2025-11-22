import clsx from "clsx";
import type { File } from "@sd/ts-client";
import { File as FileComponent } from "../../File";
import { useExplorer } from "../../context";
import { useSelection } from "../../SelectionContext";
import { formatBytes, formatRelativeTime } from "../../utils";
import { TagPill } from "../../../Tags";

interface FileRowProps {
  file: File;
  fileIndex: number;
  allFiles: File[];
}

export function FileRow({ file, fileIndex, allFiles }: FileRowProps) {
  const { setCurrentPath } = useExplorer();
  const { selectFile, isSelected } = useSelection();

  const selected = isSelected(file.id);

  const handleClick = (e: React.MouseEvent) => {
    const multi = e.metaKey || e.ctrlKey;
    const range = e.shiftKey;
    selectFile(file, allFiles, multi, range);
  };

  const handleDoubleClick = () => {
    if (file.kind === "Directory") {
      setCurrentPath(file.sd_path);
    }
  };

  return (
    <div
      onClick={handleClick}
      onDoubleClick={handleDoubleClick}
      className="flex items-center px-2 py-1.5 group"
    >
      <div
        className={clsx(
          "rounded-lg p-1.5 transition-colors mr-3",
          selected ? "bg-app-box" : "bg-transparent"
        )}
      >
        <FileComponent.Thumb file={file} size={16} />
      </div>
      <div className="flex-1 flex items-center gap-2 min-w-0">
        <div
          className={clsx(
            "text-sm truncate px-2 py-0.5 rounded-md transition-colors inline-block",
            selected ? "bg-accent text-white" : "text-ink"
          )}
        >
          {file.name}
        </div>

        {/* Tag Pills (compact) */}
        {file.tags && file.tags.length > 0 && (
          <div className="flex items-center gap-1 flex-shrink-0">
            {file.tags.slice(0, 2).map((tag) => (
              <TagPill
                key={tag.id}
                color={tag.color || '#3B82F6'}
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
      <div className="w-24 text-sm text-ink-dull">
        {file.size > 0 ? formatBytes(file.size) : "—"}
      </div>
      <div className="w-32 text-sm text-ink-dull">
        {formatRelativeTime(file.modified_at)}
      </div>
      <div className="w-24 text-sm text-ink-dull">
        {file.kind === "File" ? file.extension || "—" : "Folder"}
      </div>
    </div>
  );
}
