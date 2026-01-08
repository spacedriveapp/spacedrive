import {
  ArrowRight,
  ArrowsLeftRight,
  CircleNotch,
  Copy as CopyIcon,
  Files,
  FolderOpen,
  Warning,
} from "@phosphor-icons/react";
import type { File as FileType, SdPath } from "@sd/ts-client";
import { Dialog, dialogManager, useDialog } from "@sd/ui";
import { useEffect, useState } from "react";
import { useForm } from "react-hook-form";
import {
  useLibraryMutation,
  useLibraryQuery,
} from "../../contexts/SpacedriveContext";
import { File, FileStack } from "../../routes/explorer/File";

interface FileOperationDialogProps {
  id: number;
  operation: "copy" | "move";
  sources: SdPath[];
  destination: SdPath;
  onComplete?: () => void;
}

type ConflictResolution = "Overwrite" | "AutoModifyName" | "Skip" | "Abort";

type DialogPhase =
  | { type: "form" }
  | { type: "executing" }
  | { type: "error"; message: string };

export function useFileOperationDialog() {
  return (options: Omit<FileOperationDialogProps, "id">) => {
    return dialogManager.create((props: FileOperationDialogProps) => (
      <FileOperationDialog {...props} {...options} />
    ));
  };
}

function FileOperationDialog(props: FileOperationDialogProps) {
  const dialog = useDialog(props);
  const form = useForm();
  const [phase, setPhase] = useState<DialogPhase>({ type: "form" });
  const [operation, setOperation] = useState<"copy" | "move">(props.operation);
  const [conflictResolution, setConflictResolution] =
    useState<ConflictResolution>("Skip");

  const copyFiles = useLibraryMutation("files.copy");

  // Fetch file info for sources (up to 3 for FileStack)
  const sourcePaths = props.sources
    .slice(0, 3)
    .map((s) => ("Physical" in s ? s.Physical.path : null))
    .filter(Boolean);

  const sourceFileQueries = sourcePaths.map((path) =>
    useLibraryQuery({
      type: "files.by_path",
      input: { path },
      enabled: !!path,
    })
  );

  const sourceFiles = sourceFileQueries
    .map((q) => q.data)
    .filter((f): f is FileType => f !== undefined && f !== null);

  // Fetch destination folder info
  const destPath =
    "Physical" in props.destination ? props.destination.Physical.path : null;

  const { data: destFile } = useLibraryQuery({
    type: "files.by_path",
    input: { path: destPath },
    enabled: !!destPath,
  });

  // Check if any source is the same as destination
  const hasSameSourceDest = props.sources.some((source) => {
    if ("Physical" in source && "Physical" in props.destination) {
      return source.Physical.path === props.destination.Physical.path;
    }
    return false;
  });

  // Auto-close if invalid operation (must be in useEffect to avoid render loop)
  useEffect(() => {
    if (hasSameSourceDest) {
      dialogManager.setState(props.id, { open: false });
    }
  }, [hasSameSourceDest, props.id]);

  if (hasSameSourceDest) {
    return null;
  }

  const handleSubmit = async () => {
    try {
      setPhase({ type: "executing" });

      // Execute with the user's chosen operation and conflict resolution
      await copyFiles.mutateAsync({
        sources: { paths: props.sources },
        destination: props.destination,
        overwrite: conflictResolution === "Overwrite",
        verify_checksum: false,
        preserve_timestamps: true,
        move_files: operation === "move",
        copy_method: "Auto",
        on_conflict: conflictResolution,
      });

      // Close immediately on success
      dialogManager.setState(props.id, { open: false });
      props.onComplete?.();
    } catch (error) {
      setPhase({
        type: "error",
        message: error instanceof Error ? error.message : "Operation failed",
      });
    }
  };

  const handleCancel = () => {
    dialogManager.setState(props.id, { open: false });
  };

  // Keyboard shortcuts
  useEffect(() => {
    if (phase.type !== "form") return;

    const handleKeyDown = (e: KeyboardEvent) => {
      // Enter - Submit
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        handleSubmit();
        return;
      }

      // Only handle other shortcuts if not typing in an input
      if ((e.target as HTMLElement)?.tagName === "INPUT") return;

      // ⌘1 / Ctrl+1 - Copy mode
      if ((e.metaKey || e.ctrlKey) && e.key === "1") {
        e.preventDefault();
        e.stopPropagation();
        setOperation("copy");
      }
      // ⌘2 / Ctrl+2 - Move mode
      if ((e.metaKey || e.ctrlKey) && e.key === "2") {
        e.preventDefault();
        e.stopPropagation();
        setOperation("move");
      }
      // S - Skip
      if (e.key === "s" && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        setConflictResolution("Skip");
      }
      // K - Keep both
      if (e.key === "k" && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        setConflictResolution("AutoModifyName");
      }
      // O - Overwrite
      if (e.key === "o" && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        setConflictResolution("Overwrite");
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [phase.type, operation, conflictResolution]);

  // Executing state
  if (phase.type === "executing") {
    return (
      <Dialog
        dialog={dialog}
        form={form}
        hideButtons
        icon={<Files size={20} weight="bold" />}
        title={operation === "copy" ? "Copying Files" : "Moving Files"}
      >
        <div className="space-y-3 py-6">
          <div className="flex items-center justify-center gap-3">
            <CircleNotch
              className="size-6 animate-spin text-accent"
              weight="bold"
            />
            <span className="text-ink text-sm">
              {operation === "copy" ? "Copying files..." : "Moving files..."}
            </span>
          </div>
        </div>
      </Dialog>
    );
  }

  // Error state
  if (phase.type === "error") {
    return (
      <Dialog
        ctaLabel="Close"
        dialog={dialog}
        form={form}
        icon={<Warning className="text-red-500" size={20} weight="fill" />}
        onSubmit={handleCancel}
        title="Operation Failed"
      >
        <div className="flex flex-col gap-4 py-4">
          <div className="flex items-start gap-2 rounded-md border border-red-500/20 bg-red-500/10 p-3">
            <Warning className="mt-0.5 size-5 text-red-500" weight="fill" />
            <div className="flex-1">
              <div className="mb-1 font-medium text-ink text-sm">Error</div>
              <div className="text-ink-dull text-xs">{phase.message}</div>
            </div>
          </div>
        </div>
      </Dialog>
    );
  }

  const sourceCount = props.sources.length;
  const pluralItems = sourceCount === 1 ? "item" : "items";

  // Form state - let user choose operation and conflict resolution
  return (
    <Dialog
      ctaLabel={operation === "copy" ? "Copy" : "Move"}
      dialog={dialog}
      form={form}
      formClassName="!min-w-[400px] !max-w-[400px]"
      icon={<Files size={20} weight="bold" />}
      onCancelled={handleCancel}
      onSubmit={handleSubmit}
      title="File Operation"
    >
      <div className="space-y-5 py-2">
        {/* Source → Destination visual */}
        <div className="flex items-center gap-4">
          {/* Source */}
          <div className="flex min-w-0 flex-1 flex-col items-center gap-2">
            {sourceFiles.length > 0 ? (
              <>
                {sourceFiles.length === 1 ? (
                  <File.Thumb file={sourceFiles[0]} size={80} />
                ) : (
                  <FileStack files={sourceFiles} size={80} />
                )}
                <div className="w-full text-center">
                  <div className="mb-0.5 text-ink-dull text-xs">Source</div>
                  {sourceFiles.length === 1 ? (
                    <div className="w-full truncate font-medium text-ink text-sm">
                      {sourceFiles[0].name}
                    </div>
                  ) : (
                    <div className="font-medium text-ink text-sm">
                      {sourceCount} {pluralItems}
                    </div>
                  )}
                </div>
              </>
            ) : (
              <>
                <Files className="size-20 text-ink-dull" weight="fill" />
                <div className="text-center">
                  <div className="mb-0.5 text-ink-dull text-xs">Source</div>
                  <div className="font-medium text-ink text-sm">
                    {sourceCount} {pluralItems}
                  </div>
                </div>
              </>
            )}
          </div>

          {/* Arrow */}
          <div className="flex-shrink-0">
            <ArrowRight className="size-6 text-accent" weight="bold" />
          </div>

          {/* Destination */}
          <div className="flex min-w-0 flex-1 flex-col items-center gap-2">
            {destFile ? (
              <>
                <File.Thumb file={destFile} size={80} />
                <div className="w-full text-center">
                  <div className="mb-0.5 text-ink-dull text-xs">To</div>
                  <div className="w-full truncate font-medium text-ink text-sm">
                    {destFile.name}
                  </div>
                </div>
              </>
            ) : (
              <>
                <FolderOpen className="size-20 text-accent" weight="fill" />
                <div className="text-center">
                  <div className="mb-0.5 text-ink-dull text-xs">To</div>
                  <div className="max-w-full truncate font-medium text-ink text-sm">
                    {getFileName(props.destination)}
                  </div>
                </div>
              </>
            )}
          </div>
        </div>

        {/* Operation type selection */}
        <div className="space-y-2">
          <div className="mb-2 font-medium text-ink-dull text-xs">
            Operation:
          </div>
          <div className="flex gap-2">
            <button
              className={`flex flex-1 items-center justify-center gap-2 rounded-md px-3 py-2 font-medium text-sm transition-colors ${
                operation === "copy"
                  ? "bg-accent text-white"
                  : "bg-app-box text-ink hover:bg-app-hover"
              }`}
              onClick={() => setOperation("copy")}
              type="button"
            >
              <CopyIcon className="size-4" weight="bold" />
              Copy
              <span className="text-xs opacity-60">⌘1</span>
            </button>
            <button
              className={`flex flex-1 items-center justify-center gap-2 rounded-md px-3 py-2 font-medium text-sm transition-colors ${
                operation === "move"
                  ? "bg-accent text-white"
                  : "bg-app-box text-ink hover:bg-app-hover"
              }`}
              onClick={() => setOperation("move")}
              type="button"
            >
              <ArrowsLeftRight className="size-4" weight="bold" />
              Move
              <span className="text-xs opacity-60">⌘2</span>
            </button>
          </div>
        </div>

        {/* Conflict resolution options */}
        <div className="space-y-2">
          <div className="mb-2 font-medium text-ink-dull text-xs">
            If files already exist:
          </div>
          <div className="space-y-1">
            {[
              { value: "Skip", label: "Skip existing files", key: "S" },
              {
                value: "AutoModifyName",
                label: "Keep both (rename new files)",
                key: "K",
              },
              {
                value: "Overwrite",
                label: "Overwrite existing files",
                key: "O",
              },
            ].map((option) => (
              <label
                className="flex cursor-pointer items-center justify-between gap-2 rounded-md px-2 py-2 transition-colors hover:bg-app-hover"
                key={option.value}
              >
                <div className="flex items-center gap-2">
                  <input
                    checked={conflictResolution === option.value}
                    className="size-4 cursor-pointer accent-accent"
                    name="conflict-resolution"
                    onChange={() =>
                      setConflictResolution(option.value as ConflictResolution)
                    }
                    type="radio"
                    value={option.value}
                  />
                  <span className="text-ink text-sm">{option.label}</span>
                </div>
                <span className="font-medium text-ink-faint text-xs">
                  {option.key}
                </span>
              </label>
            ))}
          </div>
        </div>
      </div>
    </Dialog>
  );
}

// Utility functions
function getFileName(path: SdPath): string {
  if (!path || typeof path !== "object") {
    return "Unknown";
  }

  if ("Physical" in path && path.Physical) {
    const pathStr = path.Physical.path || "";
    const parts = pathStr.split("/");
    return parts[parts.length - 1] || pathStr;
  }

  if ("Cloud" in path && path.Cloud) {
    const pathStr = path.Cloud.path || "";
    const parts = pathStr.split("/");
    return parts[parts.length - 1] || pathStr;
  }

  return "Unknown";
}

function formatDestination(path: SdPath): string {
  if (!path || typeof path !== "object") {
    return "Unknown";
  }

  if ("Physical" in path && path.Physical) {
    return path.Physical.path || "Unknown";
  }

  if ("Cloud" in path && path.Cloud) {
    return path.Cloud.path || "Unknown";
  }

  if ("Content" in path && path.Content) {
    return `Content: ${path.Content.content_id}`;
  }

  if ("Sidecar" in path && path.Sidecar) {
    return `Sidecar: ${path.Sidecar.entry_id}`;
  }

  return "Unknown";
}
