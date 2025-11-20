import { useExplorer } from "../../context";
import { useNormalizedCache } from "../../../../context";
import { FileRow } from "./FileRow";
import type { DirectorySortBy } from "@sd/ts-client/generated/types";

export function ListView() {
  const { currentPath, sortBy, selectedFiles, selectFile } = useExplorer();

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
    <div className="flex flex-col p-6">
      <div className="flex items-center px-2 py-1 text-xs font-semibold text-ink-dull border-b border-app-line mb-2">
        <div className="w-10"></div>
        <div className="flex-1">Name</div>
        <div className="w-24">Size</div>
        <div className="w-32">Modified</div>
        <div className="w-24">Type</div>
      </div>

      {files.map((file) => (
        <FileRow
          key={file.id}
          file={file}
          files={files}
          selected={selectedFiles.some((f) => f.id === file.id)}
          onSelect={selectFile}
        />
      ))}
    </div>
  );
}
