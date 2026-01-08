import type { File } from "@sd/ts-client";
import clsx from "clsx";
import { useRef } from "react";
import {
  clearDragData,
  type SidebarDragData,
  setDragData,
} from "../../../../components/SpacesSidebar/dnd";
import { usePlatform } from "../../../../contexts/PlatformContext";
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
  if (file.kind === "Directory") return "bg-accent";

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
  if (
    ["js", "ts", "jsx", "tsx", "py", "rs", "go", "java", "cpp"].includes(ext)
  ) {
    return "bg-green-500";
  }

  // Archives
  if (["zip", "tar", "gz", "rar", "7z"].includes(ext)) {
    return "bg-yellow-500";
  }

  return "bg-accent";
}

export function SizeCircle({
  file,
  diameter,
  selected,
  onSelect,
}: SizeCircleProps) {
  const platform = usePlatform();
  const dragStartPos = useRef<{ x: number; y: number } | null>(null);
  const isDraggingRef = useRef(false);

  const handleClick = (e: React.MouseEvent) => {
    const multi = e.metaKey || e.ctrlKey;
    const range = e.shiftKey;
    onSelect(file, multi, range);
  };

  const handleMouseDown = (e: React.MouseEvent) => {
    if (e.button === 0) {
      dragStartPos.current = { x: e.clientX, y: e.clientY };
    }
  };

  const handleMouseMove = async (e: React.MouseEvent) => {
    if (!dragStartPos.current || isDraggingRef.current) return;
    if (!platform.startDrag) return;

    const dx = e.clientX - dragStartPos.current.x;
    const dy = e.clientY - dragStartPos.current.y;
    const distance = Math.sqrt(dx * dx + dy * dy);

    if (distance > 8) {
      isDraggingRef.current = true;

      const dragData: SidebarDragData = {
        type: "explorer-file",
        sdPath: file.sd_path,
        name: file.name,
      };
      setDragData(dragData);

      let filePath = "";
      if ("Physical" in file.sd_path) {
        filePath = file.sd_path.Physical.path;
      }

      try {
        await platform.startDrag({
          items: [
            {
              id: file.id,
              kind: filePath
                ? { type: "file", path: filePath }
                : { type: "text", content: file.name },
            },
          ],
          allowedOperations: ["copy", "move"],
        });
      } catch (err) {
        console.error("Failed to start drag:", err);
      }

      dragStartPos.current = null;
      isDraggingRef.current = false;
      clearDragData();
    }
  };

  const handleMouseUp = () => {
    dragStartPos.current = null;
    isDraggingRef.current = false;
  };

  const handleMouseLeave = () => {
    if (!isDraggingRef.current) {
      dragStartPos.current = null;
    }
  };

  const color = getFileColor(file);
  const type = getFileType(file);

  return (
    <div
      className="group flex cursor-pointer flex-col items-center gap-2"
      onClick={handleClick}
      onMouseDown={handleMouseDown}
      onMouseLeave={handleMouseLeave}
      onMouseMove={handleMouseMove}
      onMouseUp={handleMouseUp}
      style={{ width: `${diameter}px` }}
    >
      <div
        className={clsx(
          "flex flex-col items-center justify-center rounded-full transition-all",
          "shadow-lg hover:shadow-xl",
          color,
          selected
            ? "scale-105 ring-4 ring-accent"
            : "ring-2 ring-transparent hover:scale-105"
        )}
        style={{
          width: `${diameter}px`,
          height: `${diameter}px`,
        }}
      >
        <div className="px-4 text-center font-bold text-white">
          <div
            className="max-w-full truncate"
            style={{
              fontSize:
                diameter > 200 ? "16px" : diameter > 120 ? "14px" : "12px",
            }}
          >
            {file.name}
          </div>
          <div
            className="mt-1 text-white/80"
            style={{
              fontSize:
                diameter > 200 ? "14px" : diameter > 120 ? "12px" : "10px",
            }}
          >
            {type}
          </div>
          <div
            className="mt-2 font-semibold"
            style={{
              fontSize:
                diameter > 200 ? "18px" : diameter > 120 ? "16px" : "14px",
            }}
          >
            {formatBytes(file.size)}
          </div>
        </div>
      </div>
    </div>
  );
}
