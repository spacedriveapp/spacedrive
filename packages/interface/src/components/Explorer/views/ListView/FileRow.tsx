import clsx from "clsx";
import type { File } from "@sd/ts-client";
import { File as FileComponent } from "../../File";
import { useExplorer } from "../../context";
import { useSelection } from "../../SelectionContext";
import { formatBytes, formatRelativeTime } from "../../utils";

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
    if (file.kind.type === "Directory") {
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
      <div className="flex-1 flex items-center">
        <div
          className={clsx(
            "text-sm truncate px-2 py-0.5 rounded-md transition-colors inline-block max-w-full",
            selected ? "bg-accent text-white" : "text-ink"
          )}
        >
          {file.name}
        </div>
      </div>
      <div className="w-24 text-sm text-ink-dull">
        {file.size > 0 ? formatBytes(file.size) : "—"}
      </div>
      <div className="w-32 text-sm text-ink-dull">
        {formatRelativeTime(file.modified_at)}
      </div>
      <div className="w-24 text-sm text-ink-dull">
        {file.kind.type === "File" ? file.kind.data?.extension || "—" : "Folder"}
      </div>
    </div>
  );
}
