import { useExplorer } from "../../context";
import { useSelection } from "../../SelectionContext";
import { useNormalizedCache } from "../../../../context";
import { FileCard } from "./FileCard";
import type { DirectorySortBy } from "@sd/ts-client";

export function GridView() {
  const { currentPath, sortBy, viewSettings } = useExplorer();
  const { isSelected, focusedIndex, selectedFiles, selectFile, clearSelection } = useSelection();
  const { gridSize, gapSize } = viewSettings;

  const directoryQuery = useNormalizedCache({
    wireMethod: "query:files.directory_listing",
    input: currentPath
      ? {
          path: currentPath,
          limit: null,
          include_hidden: false,
          sort_by: sortBy as DirectorySortBy,
        }
      : null!,
    resourceType: "file",
    enabled: !!currentPath,
    pathScope: currentPath ?? undefined,
  });

  const files = directoryQuery.data?.files || [];

  const handleContainerClick = (e: React.MouseEvent) => {
    if (e.target === e.currentTarget) {
      clearSelection();
    }
  };

  return (
    <div
      className="grid p-3 min-h-full"
      style={{
        gridTemplateColumns: `repeat(auto-fill, minmax(${gridSize}px, 1fr))`,
        gridAutoRows: 'max-content',
        gap: `${gapSize}px`,
      }}
      onClick={handleContainerClick}
    >
      {files.map((file, index) => (
        <FileCard
          key={file.id}
          file={file}
          fileIndex={index}
          allFiles={files}
          selected={isSelected(file.id)}
          focused={index === focusedIndex}
          selectedFiles={selectedFiles}
          selectFile={selectFile}
        />
      ))}
    </div>
  );
}
