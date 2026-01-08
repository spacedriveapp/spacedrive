import { Folder } from "@sd/assets/icons";
import type { File } from "@sd/ts-client";
import { useMemo } from "react";
import { useNormalizedQuery } from "../../contexts/SpacedriveContext";
import { File as FileComponent } from "../../routes/explorer/File";

interface DirectoryPreviewProps {
  file: File;
}

export function DirectoryPreview({ file }: DirectoryPreviewProps) {
  const directoryQuery = useNormalizedQuery({
    wireMethod: "query:files.directory_listing",
    input: {
      path: file.sd_path,
      limit: null,
      include_hidden: false,
      sort_by: "modified" as any,
      folders_first: true,
    },
    resourceType: "file",
    pathScope: file.sd_path,
    enabled: true,
  });

  const allFiles = (directoryQuery.data as any)?.files || [];

  const directories = useMemo(() => {
    return allFiles;
  }, [allFiles]);

  const gridSize = 120;
  const gapSize = 12;

  if (directoryQuery.isLoading) {
    return (
      <div className="flex h-full w-full items-center justify-center">
        <div className="text-center">
          <img
            alt="Folder Icon"
            className="mx-auto mb-4 h-16 w-16"
            src={Folder}
          />
          <div className="font-medium text-ink text-lg">{file.name}</div>
          <div className="mt-2 text-ink-dull text-sm">
            Loading directories...
          </div>
        </div>
      </div>
    );
  }

  if (directories.length === 0) {
    return (
      <div className="flex h-full w-full items-center justify-center">
        <div className="text-center">
          <img
            alt="Folder Icon"
            className="mx-auto mb-4 h-16 w-16"
            src={Folder}
          />
          <div className="font-medium text-ink text-lg">{file.name}</div>
          <div className="mt-2 text-ink-dull text-sm">No subdirectories</div>
        </div>
      </div>
    );
  }

  const thumbSize = Math.max(gridSize * 0.6, 60);

  return (
    <div className="h-full w-full overflow-auto">
      <div
        className="grid p-6"
        style={{
          gridTemplateColumns: `repeat(auto-fill, minmax(${gridSize}px, 1fr))`,
          gridAutoRows: "max-content",
          gap: `${gapSize}px`,
        }}
      >
        {directories.map((dir) => (
          <div
            className="flex flex-col items-center gap-2 rounded-lg p-1 hover:bg-app-hover/20"
            key={dir.id}
          >
            <div className="rounded-lg p-2">
              <FileComponent.Thumb file={dir} size={thumbSize} />
            </div>
            <div className="flex w-full flex-col items-center">
              <div className="inline-block max-w-full truncate rounded-md px-2 py-0.5 text-ink text-sm">
                {dir.name}
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
