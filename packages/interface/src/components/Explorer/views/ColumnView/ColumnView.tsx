import { useState, useEffect, useMemo } from "react";
import type { SdPath } from "@sd/ts-client/generated/types";
import { useExplorer } from "../../context";
import { Column } from "./Column";

export function ColumnView() {
  const { currentPath, setCurrentPath, selectedFiles } = useExplorer();
  const [columnStack, setColumnStack] = useState<SdPath[]>([]);

  // Initialize column stack when currentPath changes
  useEffect(() => {
    if (currentPath) {
      // Start with just the current path
      setColumnStack([currentPath]);
    }
  }, [currentPath]);

  // Add a column when a directory is selected
  useEffect(() => {
    if (selectedFiles.length === 1 && selectedFiles[0].kind === "Directory") {
      const selectedDir = selectedFiles[0];
      const selectedPath = selectedDir.sd_path;

      // Find which column this file's parent is in
      const parentPath = selectedDir.sd_path.Physical?.path;
      if (!parentPath) return;

      const parentDir = parentPath.substring(0, parentPath.lastIndexOf('/'));

      const parentColumnIndex = columnStack.findIndex((p) => {
        if (p.Physical && selectedPath.Physical) {
          return (
            p.Physical.device_slug === selectedPath.Physical.device_slug &&
            p.Physical.path === parentDir
          );
        }
        return false;
      });

      if (parentColumnIndex !== -1) {
        // Found the parent column - truncate everything after it and add new column
        setColumnStack((prev) => [...prev.slice(0, parentColumnIndex + 1), selectedPath]);
      } else {
        // Fallback: just check if this exact path exists
        const existingIndex = columnStack.findIndex((p) => {
          if (p.Physical && selectedPath.Physical) {
            return (
              p.Physical.device_slug === selectedPath.Physical.device_slug &&
              p.Physical.path === selectedPath.Physical.path
            );
          }
          return false;
        });

        if (existingIndex === -1) {
          // Not in stack, add it
          setColumnStack((prev) => [...prev, selectedPath]);
        }
      }
    }
  }, [selectedFiles, columnStack]);

  const handleNavigate = (path: SdPath) => {
    setCurrentPath(path);
  };

  // Determine which column is active based on current path
  const activeColumnIndex = useMemo(() => {
    if (!currentPath) return -1;

    return columnStack.findIndex((p) => {
      if (p.Physical && currentPath.Physical) {
        return (
          p.Physical.device_slug === currentPath.Physical.device_slug &&
          p.Physical.path === currentPath.Physical.path
        );
      }
      return false;
    });
  }, [columnStack, currentPath]);

  if (!currentPath) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-ink-dull">No location selected</div>
      </div>
    );
  }

  return (
    <div className="flex h-full overflow-x-auto bg-app">
      {columnStack.map((path, index) => (
        <Column
          key={`${path.Physical?.device_slug}-${path.Physical?.path}-${index}`}
          path={path}
          isActive={index === activeColumnIndex}
          onNavigate={handleNavigate}
        />
      ))}
    </div>
  );
}
