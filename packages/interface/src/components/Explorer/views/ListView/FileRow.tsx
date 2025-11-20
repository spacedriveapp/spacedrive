import clsx from "clsx";
import type { File } from "@sd/ts-client/generated/types";
import { File as FileComponent } from "../../File";
import { useExplorer } from "../../context";
import { formatBytes, formatRelativeTime } from "../../utils";

interface FileRowProps {
  file: File;
  files: File[];
  selected: boolean;
  onSelect: (file: File, files: File[], multi?: boolean, range?: boolean) => void;
}

export function FileRow({ file, files, selected, onSelect }: FileRowProps) {
  const { setCurrentPath } = useExplorer();

  const handleClick = (e: React.MouseEvent) => {
    const multi = e.metaKey || e.ctrlKey;
    const range = e.shiftKey;
    onSelect(file, files, multi, range);
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
        {file.extension || "Folder"}
      </div>
    </div>
  );
}
