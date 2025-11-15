import clsx from "clsx";
import { Copy, Trash, Eye, FolderOpen, MagnifyingGlass } from "@phosphor-icons/react";
import type { File } from "@sd/ts-client/generated/types";
import { File as FileComponent } from "../../File";
import { useExplorer } from "../../context";
import { useContextMenu } from "../../../../hooks/useContextMenu";
import { useLibraryMutation } from "../../../../context";
import { usePlatform } from "../../../../platform";
import { formatBytes } from "../../utils";

interface FileCardProps {
  file: File;
  selected: boolean;
  focused: boolean;
  onSelect: (file: File, multi?: boolean, range?: boolean) => void;
}

export function FileCard({ file, selected, focused, onSelect }: FileCardProps) {
  const { setCurrentPath, viewSettings, selectedFiles, currentPath } = useExplorer();
  const { gridSize, showFileSize } = viewSettings;
  const platform = usePlatform();
  const copyFiles = useLibraryMutation("files.copy");
  const deleteFiles = useLibraryMutation("files.delete");

  // Get the files to operate on (multi-select or just this file)
  const getTargetFiles = () => {
    if (selected && selectedFiles.length > 0) {
      return selectedFiles;
    }
    return [file];
  };

  const contextMenu = useContextMenu({
    items: [
      {
        icon: Eye,
        label: "Quick Look",
        onClick: () => {
          console.log("Quick Look:", file.name);
          // TODO: Implement quick look
        },
        keybind: "Space",
      },
      {
        icon: FolderOpen,
        label: "Open",
        onClick: () => {
          if (file.kind === "Directory") {
            setCurrentPath(file.sd_path);
          } else {
            console.log("Open file:", file.name);
            // TODO: Implement file opening
          }
        },
        keybind: "⌘O",
        condition: () => file.kind === "Directory" || file.kind === "File",
      },
      {
        icon: MagnifyingGlass,
        label: "Show in Finder",
        onClick: async () => {
          // Extract the physical path from SdPath
          if ("Physical" in file.sd_path) {
            const physicalPath = file.sd_path.Physical.path;
            if (platform.revealFile) {
              try {
                await platform.revealFile(physicalPath);
              } catch (err) {
                console.error("Failed to reveal file:", err);
                alert(`Failed to reveal file: ${err}`);
              }
            } else {
              console.log("revealFile not supported on this platform");
            }
          } else {
            console.log("Cannot reveal non-physical file");
          }
        },
        keybind: "⌘⇧R",
        condition: () => "Physical" in file.sd_path && !!platform.revealFile,
      },
      { type: "separator" },
      {
        icon: Copy,
        label: selected && selectedFiles.length > 1 ? `Copy ${selectedFiles.length} items` : "Copy",
        onClick: async () => {
          const targets = getTargetFiles();
          const sdPaths = targets.map(f => f.sd_path);

          console.log("Copying files:", targets.map(f => f.name));

          // Store the file paths for paste
          window.__SPACEDRIVE__ = window.__SPACEDRIVE__ || {};
          window.__SPACEDRIVE__.clipboard = {
            operation: 'copy',
            files: sdPaths,
            sourcePath: currentPath,
          };

          console.log(`Copied ${sdPaths.length} files to clipboard`);
        },
        keybind: "⌘C",
      },
      {
        icon: Copy,
        label: "Paste",
        onClick: async () => {
          const clipboard = window.__SPACEDRIVE__?.clipboard;
          if (!clipboard || !clipboard.files || !currentPath) {
            console.log("Nothing to paste or no destination");
            return;
          }

          console.log(`Pasting ${clipboard.files.length} files to:`, currentPath);

          try {
            console.log("Paste params:", {
              sources: clipboard.files,
              destination: currentPath,
            });

            const result = await copyFiles.mutateAsync({
              sources: { paths: clipboard.files },
              destination: currentPath,
              overwrite: false,
              verify_checksum: false,
              preserve_timestamps: true,
              move_files: false,
              copy_method: "Auto" as const,
            });

            console.log("Paste operation result:", result);
            console.log("Result type:", typeof result, result);

            // Check if it's a confirmation request
            if (result && typeof result === 'object' && 'NeedsConfirmation' in result) {
              console.log("Action needs confirmation:", result);
              alert("File conflict detected - confirmation UI not implemented yet");
            } else if (result && typeof result === 'object' && 'job_id' in result) {
              console.log("Job started with ID:", result.job_id);
            }
          } catch (err) {
            console.error("Failed to paste:", err);
            alert(`Failed to paste: ${err}`);
          }
        },
        keybind: "⌘V",
        condition: () => {
          const clipboard = window.__SPACEDRIVE__?.clipboard;
          return !!clipboard && !!clipboard.files && clipboard.files.length > 0;
        },
      },
      { type: "separator" },
      {
        icon: Trash,
        label: selected && selectedFiles.length > 1 ? `Delete ${selectedFiles.length} items` : "Delete",
        onClick: async () => {
          const targets = getTargetFiles();
          const message = targets.length > 1
            ? `Delete ${targets.length} items?`
            : `Delete "${file.name}"?`;

          if (confirm(message)) {
            console.log("Deleting files:", targets.map(f => f.name));

            try {
              const result = await deleteFiles.mutateAsync({
                targets: { paths: targets.map(f => f.sd_path) },
                permanent: false, // Move to trash, not permanent delete
                recursive: true,  // Allow deleting non-empty directories
              });
              console.log("Delete operation started:", result);
            } catch (err) {
              console.error("Failed to delete:", err);
              alert(`Failed to delete: ${err}`);
            }
          }
        },
        keybind: "⌘⌫",
        variant: "danger" as const,
      },
    ],
  });

  const handleClick = (e: React.MouseEvent) => {
    const multi = e.metaKey || e.ctrlKey;
    const range = e.shiftKey;
    console.log("FileCard clicked:", file.name, "multi:", multi, "range:", range, "currently selected:", selected);
    onSelect(file, multi, range);
  };

  const handleDoubleClick = () => {
    if (file.kind === "Directory") {
      setCurrentPath(file.sd_path);
    }
  };

  const handleContextMenu = async (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();

    // Select the file if not already selected
    if (!selected) {
      onSelect(file, false, false);
    }

    await contextMenu.show(e);
  };

  const thumbSize = Math.max(gridSize * 0.6, 60);

  return (
    <FileComponent
      file={file}
      selected={selected}
      onClick={handleClick}
      onDoubleClick={handleDoubleClick}
      onContextMenu={handleContextMenu}
      layout="column"
      className={clsx(
        "flex flex-col items-center gap-2 p-1 rounded-lg transition-all",
        focused && !selected && "ring-2 ring-accent/50"
      )}
    >
      <div
        className={clsx(
          "rounded-lg p-2",
          selected ? "bg-app-box" : "bg-transparent"
        )}
      >
        <FileComponent.Thumb file={file} size={thumbSize} />
      </div>
      <div className="w-full flex flex-col items-center">
        <div
          className={clsx(
            "text-sm truncate px-2 py-0.5 rounded-md inline-block max-w-full",
            selected ? "bg-accent text-white" : "text-ink"
          )}
        >
          {file.name}
        </div>
        {showFileSize && file.size > 0 && (
          <div className="text-xs text-ink-dull mt-0.5">
            {formatBytes(file.size)}
          </div>
        )}
      </div>
    </FileComponent>
  );
}
