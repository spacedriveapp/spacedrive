import { useExplorer } from "../../context";
import { useNormalizedCache } from "../../../../context";
import { FileCard } from "./FileCard";
import type { DirectorySortBy } from "@sd/ts-client/generated/types";

export function GridView() {
  const { currentPath, sortBy, viewSettings } = useExplorer();
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
        />
      ))}
    </div>
  );
}
