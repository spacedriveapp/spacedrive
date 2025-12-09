import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import type { SdPath, File } from "@sd/ts-client";
import { useExplorer } from "../../context";
import { useSelection } from "../../SelectionContext";
import { useNormalizedQuery } from "../../../../context";
import type { DirectorySortBy } from "@sd/ts-client";
import { Column } from "./Column";

export function ColumnView() {
  const { currentPath, setCurrentPath, sortBy, viewSettings } = useExplorer();
  const { selectedFiles, selectFile, clearSelection } = useSelection();
  const [columnStack, setColumnStack] = useState<SdPath[]>([]);
  const isInternalNavigationRef = useRef(false);

  // Initialize column stack when currentPath changes externally
  useEffect(() => {
    // Only reset if this is an external navigation (not from within column view)
    if (currentPath && !isInternalNavigationRef.current) {
      setColumnStack([currentPath]);
      clearSelection();
    }
    isInternalNavigationRef.current = false;
  }, [currentPath, clearSelection]);

  // Handle file selection - uses global selectFile and updates columns
  const handleSelectFile = useCallback((file: File, columnIndex: number, files: File[], multi = false, range = false) => {
    // Use global selectFile to update selection state
    selectFile(file, files, multi, range);

    // Only update columns for single directory selection
    if (!multi && !range) {
      if (file.kind === "Directory") {
        // Truncate columns after current and add new one
        setColumnStack((prev) => [...prev.slice(0, columnIndex + 1), file.sd_path]);
        // Update currentPath to the selected directory
        isInternalNavigationRef.current = true;
        setCurrentPath(file.sd_path);
      } else {
        // For files, just truncate columns after current
        setColumnStack((prev) => prev.slice(0, columnIndex + 1));
        // Update currentPath to the file's parent directory
        const parentPath = columnStack[columnIndex];
        if (parentPath) {
          isInternalNavigationRef.current = true;
          setCurrentPath(parentPath);
        }
      }
    }
  }, [selectFile, setCurrentPath, columnStack]);

  const handleNavigate = useCallback((path: SdPath) => {
    setCurrentPath(path);
  }, [setCurrentPath]);

  // Find the active column (the one containing the first selected file)
  const activeColumnIndex = useMemo(() => {
    if (selectedFiles.length === 0) return columnStack.length - 1; // Default to last column

    const firstSelected = selectedFiles[0];
    const filePath = firstSelected.sd_path.Physical?.path;
    if (!filePath) return columnStack.length - 1;

    const fileParent = filePath.substring(0, filePath.lastIndexOf('/'));

    return columnStack.findIndex((path) => {
      const columnPath = path.Physical?.path;
      return columnPath === fileParent;
    });
  }, [selectedFiles, columnStack]);

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

        const currentIndex = selectedFiles.length > 0
          ? activeColumnFiles.findIndex((f) => f.id === selectedFiles[0].id)
          : -1;

        const newIndex = e.key === "ArrowDown"
          ? currentIndex < 0 ? 0 : Math.min(currentIndex + 1, activeColumnFiles.length - 1)
          : currentIndex < 0 ? 0 : Math.max(currentIndex - 1, 0);

        if (newIndex !== currentIndex && activeColumnFiles[newIndex]) {
          const newFile = activeColumnFiles[newIndex];
          handleSelectFile(newFile, activeColumnIndex, activeColumnFiles);

          // Scroll to keep selection visible
          const element = document.querySelector(`[data-file-id="${newFile.id}"]`);
          if (element) {
            element.scrollIntoView({ block: "nearest", behavior: "smooth" });
          }
        }
      } else if (e.key === "ArrowLeft") {
        // Move to previous column
        if (activeColumnIndex > 0) {
          const previousColumnPath = columnStack[activeColumnIndex - 1];
          // Truncate columns and stay at previous column
          setColumnStack((prev) => prev.slice(0, activeColumnIndex));
          clearSelection();
          // Update currentPath to previous column
          if (previousColumnPath) {
            isInternalNavigationRef.current = true;
            setCurrentPath(previousColumnPath);
          }
        }
      } else if (e.key === "ArrowRight") {
        // If selected file is a directory and there's a next column, move focus there
        const firstSelected = selectedFiles[0];
        if (firstSelected?.kind === "Directory" && activeColumnIndex < columnStack.length - 1) {
          // Select first item in next column
          if (nextColumnFiles.length > 0) {
            const firstFile = nextColumnFiles[0];
            handleSelectFile(firstFile, activeColumnIndex + 1, nextColumnFiles);

            // Scroll to keep selection visible
            setTimeout(() => {
              const element = document.querySelector(`[data-file-id="${firstFile.id}"]`);
              if (element) {
                element.scrollIntoView({ block: "nearest", behavior: "smooth" });
              }
            }, 0);
          }
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [activeColumnFiles, nextColumnFiles, selectedFiles, activeColumnIndex, columnStack, handleSelectFile]);

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
        // A column is active if it contains a selected file or is the last column with no selection
        const isActive = selectedFiles.length > 0
          ? // Check if any selected file's parent path matches this column's path
            selectedFiles.some((file) => {
              const filePath = file.sd_path.Physical?.path;
              const columnPath = path.Physical?.path;
              if (!filePath || !columnPath) return false;
              const fileParent = filePath.substring(0, filePath.lastIndexOf('/'));
              return fileParent === columnPath;
            })
          : index === columnStack.length - 1; // Last column is active if no selection

        return (
          <Column
            key={`${path.Physical?.device_slug}-${path.Physical?.path}-${index}`}
            path={path}
            selectedFiles={selectedFiles}
            onSelectFile={(file, files, multi, range) => handleSelectFile(file, index, files, multi, range)}
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
