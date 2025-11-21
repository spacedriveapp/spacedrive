import { useExplorer } from "../../context";
import { useSelection } from "../../SelectionContext";
import { useNormalizedCache } from "../../../../context";
import { FileCard } from "./FileCard";
import type { DirectorySortBy } from "@sd/ts-client";

export function GridView() {
  const { currentPath, sortBy, viewSettings } = useExplorer();
  const { isSelected, focusedIndex, selectedFiles, selectFile } = useSelection();
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

  return (
    <div
      className="grid p-3"
      style={{
        gridTemplateColumns: `repeat(auto-fill, minmax(${gridSize}px, 1fr))`,
        gap: `${gapSize}px`,
      }}
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
