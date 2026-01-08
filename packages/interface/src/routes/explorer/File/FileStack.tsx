import type { File as FileType } from "@sd/ts-client";
import { File } from "./File";

interface FileStackProps {
  files: FileType[];
  size?: number;
}

/**
 * FileStack - Renders multiple files stacked with rotation
 * Shows up to 3 files stacked on top of each other with slight rotation
 */
export function FileStack({ files, size = 64 }: FileStackProps) {
  const displayFiles = files.slice(0, 3);
  const remainingCount = Math.max(0, files.length - 3);

  // Rotation angles for visual stacking effect
  const rotations = [-4, 0, 4];

  return (
    <div className="relative" style={{ width: size, height: size }}>
      {displayFiles.map((file, index) => (
        <div
          className="absolute inset-0 transition-transform"
          key={file.id}
          style={{
            transform: `rotate(${rotations[index]}deg) translateY(${index * -2}px)`,
            zIndex: index,
          }}
        >
          <File.Thumb file={file} size={size} />
        </div>
      ))}

      {/* Show count badge if more than 3 files */}
      {remainingCount > 0 && (
        <div className="absolute -right-1 -bottom-1 z-10 flex size-6 items-center justify-center rounded-full border-2 border-app bg-accent font-bold text-white text-xs shadow-lg">
          +{remainingCount}
        </div>
      )}
    </div>
  );
}
