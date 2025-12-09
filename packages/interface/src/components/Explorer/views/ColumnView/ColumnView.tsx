import { useState, useEffect, useCallback, useMemo } from "react";
import type { SdPath, File } from "@sd/ts-client";
import { useExplorer } from "../../context";
import { useSelection } from "../../SelectionContext";
import { useNormalizedQuery } from "../../../../context";
import type { DirectorySortBy } from "@sd/ts-client";
import { Column } from "./Column";

export function ColumnView() {
  const { currentPath, setCurrentPath, sortBy, viewSettings } = useExplorer();
  const { clearSelection, setSelectedFiles } = useSelection();
  const [columnStack, setColumnStack] = useState<SdPath[]>([]);

  // Column-specific selection state (single selection only)
  const [selectedFile, setSelectedFile] = useState<File | null>(null);

  // Sync local selection with global selection context (for inspector)
  useEffect(() => {
    if (selectedFile) {
      setSelectedFiles([selectedFile]);
    } else {
      clearSelection();
    }
  }, [selectedFile, setSelectedFiles, clearSelection]);

  // Initialize column stack when currentPath changes
  useEffect(() => {
    if (currentPath) {
      setColumnStack([currentPath]);
      setSelectedFile(null);
      clearSelection();
    }
  }, [currentPath, clearSelection]);

  // Handle file selection - updates columns
  const handleSelectFile = useCallback((file: File, columnIndex: number) => {
    setSelectedFile(file);

    // If it's a directory, add a new column
    if (file.kind === "Directory") {
      // Truncate columns after current and add new one
      setColumnStack((prev) => [...prev.slice(0, columnIndex + 1), file.sd_path]);
    } else {
      // For files, just truncate columns after current
      setColumnStack((prev) => prev.slice(0, columnIndex + 1));
    }
  }, []);

  const handleNavigate = useCallback((path: SdPath) => {
    setCurrentPath(path);
  }, [setCurrentPath]);

  // Find the active column (the one containing the selected file)
  const activeColumnIndex = useMemo(() => {
    if (!selectedFile) return columnStack.length - 1; // Default to last column

    const filePath = selectedFile.sd_path.Physical?.path;
    if (!filePath) return columnStack.length - 1;

    const fileParent = filePath.substring(0, filePath.lastIndexOf('/'));

    return columnStack.findIndex((path) => {
      const columnPath = path.Physical?.path;
      return columnPath === fileParent;
    });
  }, [selectedFile, columnStack]);

  const activeColumnPath = columnStack[activeColumnIndex];

  // Query files for the active column (for keyboard navigation)
  const activeColumnQuery = useNormalizedQuery({
    wireMethod: "query:files.directory_listing",
    input: activeColumnPath
      ? {
          path: activeColumnPath,
          limit: null,
          include_hidden: false,
          sort_by: sortBy as DirectorySortBy,
          folders_first: viewSettings.foldersFirst,
        }
      : null!,
    resourceType: "file",
    enabled: !!activeColumnPath,
    pathScope: activeColumnPath,
  });

  const activeColumnFiles = activeColumnQuery.data?.files || [];

  // Query the next column for right arrow navigation
  const nextColumnPath = columnStack[activeColumnIndex + 1];
  const nextColumnQuery = useNormalizedQuery({
    wireMethod: "query:files.directory_listing",
    input: nextColumnPath
      ? {
          path: nextColumnPath,
          limit: null,
          include_hidden: false,
          sort_by: sortBy as DirectorySortBy,
          folders_first: viewSettings.foldersFirst,
        }
      : null!,
    resourceType: "file",
    enabled: !!nextColumnPath,
    pathScope: nextColumnPath,
  });

  const nextColumnFiles = nextColumnQuery.data?.files || [];

  // Keyboard navigation
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (!["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(e.key)) {
        return;
      }

      e.preventDefault();

      if (e.key === "ArrowUp" || e.key === "ArrowDown") {
        // Navigate within current column
        if (activeColumnFiles.length === 0) return;

        const currentIndex = selectedFile
          ? activeColumnFiles.findIndex((f) => f.id === selectedFile.id)
          : -1;

        const newIndex = e.key === "ArrowDown"
          ? currentIndex < 0 ? 0 : Math.min(currentIndex + 1, activeColumnFiles.length - 1)
          : currentIndex < 0 ? 0 : Math.max(currentIndex - 1, 0);

        if (newIndex !== currentIndex && activeColumnFiles[newIndex]) {
          handleSelectFile(activeColumnFiles[newIndex], activeColumnIndex);
        }
      } else if (e.key === "ArrowLeft") {
        // Move to previous column
        if (activeColumnIndex > 0) {
          // Truncate columns and stay at previous column
          setColumnStack((prev) => prev.slice(0, activeColumnIndex));
          setSelectedFile(null);
        }
      } else if (e.key === "ArrowRight") {
        // If selected file is a directory and there's a next column, move focus there
        if (selectedFile?.kind === "Directory" && activeColumnIndex < columnStack.length - 1) {
          // Select first item in next column
          if (nextColumnFiles.length > 0) {
            handleSelectFile(nextColumnFiles[0], activeColumnIndex + 1);
          }
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [activeColumnFiles, nextColumnFiles, selectedFile, activeColumnIndex, columnStack, handleSelectFile]);

  if (!currentPath) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-ink-dull">No location selected</div>
      </div>
    );
  }

  return (
    <div className="flex h-full overflow-x-auto bg-app">
      {columnStack.map((path, index) => {
        // A column is active if it contains the selected file or is the last column with no selection
        const isActive = selectedFile
          ? // Check if selected file's parent path matches this column's path
            (() => {
              const filePath = selectedFile.sd_path.Physical?.path;
              const columnPath = path.Physical?.path;
              if (!filePath || !columnPath) return false;
              const fileParent = filePath.substring(0, filePath.lastIndexOf('/'));
              return fileParent === columnPath;
            })()
          : index === columnStack.length - 1; // Last column is active if no selection

        return (
          <Column
            key={`${path.Physical?.device_slug}-${path.Physical?.path}-${index}`}
            path={path}
            selectedFile={selectedFile}
            onSelectFile={(file) => handleSelectFile(file, index)}
            onNavigate={handleNavigate}
            nextColumnPath={columnStack[index + 1]}
            columnIndex={index}
            isActive={isActive}
          />
        );
      })}
    </div>
  );
}
