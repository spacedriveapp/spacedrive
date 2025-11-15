import clsx from "clsx";
import type { File } from "@sd/ts-client/generated/types";
import { formatBytes } from "../../utils";

interface SizeCircleProps {
  file: File;
  diameter: number;
  selected: boolean;
  onSelect: (file: File, multi?: boolean, range?: boolean) => void;
}

// Get file extension or type
function getFileType(file: File): string {
  if (file.kind === "Directory") return "Folder";

  const name = file.name;
  const lastDot = name.lastIndexOf(".");
  if (lastDot === -1 || lastDot === 0) return "File";

  return name.slice(lastDot + 1).toUpperCase();
}

// Get color based on file type
function getFileColor(file: File): string {
  if (file.kind === "Directory") return "bg-blue-500";

  const ext = file.name.split(".").pop()?.toLowerCase() || "";

  // Images
  if (["jpg", "jpeg", "png", "gif", "svg", "webp", "heic"].includes(ext)) {
    return "bg-purple-500";
  }

  // Videos
  if (["mp4", "mov", "avi", "mkv", "webm"].includes(ext)) {
    return "bg-red-500";
  }

  // Audio
  if (["mp3", "wav", "flac", "aac", "ogg"].includes(ext)) {
    return "bg-pink-500";
  }

  // Documents
  if (["pdf", "doc", "docx", "txt", "md"].includes(ext)) {
    return "bg-orange-500";
  }

  // Code
  if (["js", "ts", "jsx", "tsx", "py", "rs", "go", "java", "cpp"].includes(ext)) {
    return "bg-green-500";
  }

  // Archives
  if (["zip", "tar", "gz", "rar", "7z"].includes(ext)) {
    return "bg-yellow-500";
  }

  return "bg-accent";
}

export function SizeCircle({ file, diameter, selected, onSelect }: SizeCircleProps) {
  const handleClick = (e: React.MouseEvent) => {
    const multi = e.metaKey || e.ctrlKey;
    const range = e.shiftKey;
    onSelect(file, multi, range);
  };

  const color = getFileColor(file);
  const type = getFileType(file);

  return (
    <div
      className="flex flex-col items-center gap-2 cursor-pointer group"
      onClick={handleClick}
      style={{ width: `${diameter}px` }}
    >
      <div
        className={clsx(
          "rounded-full flex flex-col items-center justify-center transition-all",
          "shadow-lg hover:shadow-xl",
          color,
          selected ? "ring-4 ring-accent scale-105" : "ring-2 ring-transparent hover:scale-105"
        )}
        style={{
          width: `${diameter}px`,
          height: `${diameter}px`,
        }}
      >
        <div className="text-white font-bold text-center px-4">
          <div
            className="truncate max-w-full"
            style={{
              fontSize: diameter > 200 ? "16px" : diameter > 120 ? "14px" : "12px"
            }}
          >
            {file.name}
          </div>
          <div
            className="text-white/80 mt-1"
            style={{
              fontSize: diameter > 200 ? "14px" : diameter > 120 ? "12px" : "10px"
            }}
          >
            {type}
          </div>
          <div
            className="font-semibold mt-2"
            style={{
              fontSize: diameter > 200 ? "18px" : diameter > 120 ? "16px" : "14px"
            }}
          >
            {formatBytes(file.size)}
          </div>
        </div>
      </div>
    </div>
  );
}
