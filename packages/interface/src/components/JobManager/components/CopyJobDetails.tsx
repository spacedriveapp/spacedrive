import { CheckCircle, Circle, X, Spinner } from "@phosphor-icons/react";
import clsx from "clsx";
import { useEffect, useRef } from "react";
import { useLibraryQuery } from "../../../contexts/SpacedriveContext";
import type { JobListItem } from "../types";
import type { SpeedSample } from "../hooks/useJobs";
import type { File } from "@sd/ts-client";
import { SpeedGraph } from "./SpeedGraph";
import { Thumb } from "../../../routes/explorer/File/Thumb";
import { formatBytes } from "../../../routes/explorer/utils";

interface CopyJobDetailsProps {
  job: JobListItem;
  speedHistory: SpeedSample[];
}

export function CopyJobDetails({ job, speedHistory }: CopyJobDetailsProps) {
  const generic = job.generic_progress;
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const prevScrollIndexRef = useRef<number>(-1);
  const prevCompletedRef = useRef<number>(0);
  const prevCurrentPathRef = useRef<string>("");

  // Fetch copy metadata (file queue with File objects)
  const { data: metadata, refetch } = useLibraryQuery({
    type: "jobs.get_copy_metadata",
    input: { job_id: job.id },
  });

  const files = metadata?.metadata?.files || [];
  const fileObjects = metadata?.metadata?.file_objects || [];

  // Refetch when completed count changes OR when current file changes
  useEffect(() => {
    const currentCompleted = generic?.completion?.completed || 0;
    const currentPath = generic?.current_path?.Physical?.path ||
                        generic?.current_path?.Local?.path ||
                        generic?.message || "";

    const completedChanged = currentCompleted !== prevCompletedRef.current;
    const currentFileChanged = currentPath !== prevCurrentPathRef.current && currentPath !== "";

    if (completedChanged || currentFileChanged) {
      prevCompletedRef.current = currentCompleted;
      prevCurrentPathRef.current = currentPath;
      refetch();
    }
  }, [generic?.completion?.completed, generic?.current_path, generic?.message, refetch]);

  // Auto-scroll to center the currently copying file
  useEffect(() => {
    const container = scrollContainerRef.current;
    if (!container || !generic?.current_path) return;

    const currentPath = generic.current_path.Physical?.path || generic.current_path.Local?.path;
    if (!currentPath) return;

    const currentIndex = files.findIndex(f => {
      const filePath = f.source_path?.Physical?.path || f.source_path?.Local?.path;
      return filePath === currentPath;
    });

    if (currentIndex === -1 || currentIndex === prevScrollIndexRef.current) return;

    prevScrollIndexRef.current = currentIndex;

    const currentElement = container.children[currentIndex] as HTMLElement;
    if (!currentElement) return;

    const containerRect = container.getBoundingClientRect();
    const elementRect = currentElement.getBoundingClientRect();

    const elementCenter = elementRect.top + elementRect.height / 2;
    const containerCenter = containerRect.top + containerRect.height / 2;
    const scrollOffset = elementCenter - containerCenter;

    container.scrollBy({
      top: scrollOffset,
      behavior: "smooth"
    });
  }, [files, generic?.current_path]);

  if (!generic) {
    return (
      <div className="p-4 text-xs text-ink-faint">
        No progress data available
      </div>
    );
  }

  // Create a map of entry_id → File for quick lookup
  const fileMap = new Map<string, File>();
  fileObjects.forEach(file => {
    fileMap.set(file.id, file);
  });

  return (
    <div className="p-4 space-y-4">
      {/* Speed graph */}
      <SpeedGraph jobId={job.id} speedHistory={speedHistory} />

      {/* File queue */}
      {files.length > 0 && (
        <div>
          <div className="text-xs font-medium text-ink mb-2">Transfer Queue</div>
          <div
            ref={scrollContainerRef}
            className="space-y-0 max-h-[200px] overflow-y-auto border border-app-line rounded-lg"
          >
            {files.map((file, index) => {
              const fileObj = file.entry_id ? fileMap.get(file.entry_id) : null;
              const isEven = index % 2 === 0;

              return (
                <div
                  key={index}
                  className={clsx(
                    "flex items-center gap-3 px-3 py-2 transition-opacity",
                    isEven ? "bg-app/30" : "bg-app/10",
                    index === 0 && "rounded-t-lg",
                    index === files.length - 1 && "rounded-b-lg",
                    file.status === "completed" && "opacity-50"
                  )}
                >
                  {/* Status icon */}
                  {file.status === "completed" ? (
                    <CheckCircle size={14} weight="fill" className="text-ink-dull flex-shrink-0" />
                  ) : file.status === "copying" ? (
                    <Spinner size={14} className="text-accent animate-spin flex-shrink-0" />
                  ) : file.status === "failed" ? (
                    <X size={14} weight="bold" className="text-red-500 flex-shrink-0" />
                  ) : file.status === "skipped" ? (
                    <Circle size={14} weight="bold" className="text-ink-dull flex-shrink-0" />
                  ) : (
                    <Circle size={14} className="text-ink-faint flex-shrink-0" />
                  )}

                  {/* Thumbnail (if File object available) */}
                  {fileObj && (
                    <Thumb
                      file={fileObj}
                      size={32}
                      className="flex-shrink-0"
                      iconScale={0.7}
                    />
                  )}

                  {/* File name and path */}
                  <div className="flex-1 min-w-0">
                    <div className={clsx(
                      "text-xs font-medium truncate",
                      file.status === "completed" && "text-ink-dull",
                      file.status === "copying" && "text-ink",
                      file.status === "failed" && "text-red-400",
                      file.status === "skipped" && "text-ink-dull",
                      file.status === "pending" && "text-ink"
                    )}>
                      {fileObj?.name || extractFileName(file.source_path)}
                    </div>
                    <div className="text-[10px] text-ink-faint truncate font-mono">
                      {formatPath(file.source_path)}
                    </div>
                  </div>

                  {/* File size */}
                  <div className="text-[10px] text-ink-faint font-mono flex-shrink-0">
                    {formatBytes(file.size_bytes)}
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}

// Extract filename from SdPath
function extractFileName(path: any): string {
  if (typeof path === "string") {
    return path.split("/").pop() || path;
  }

  if (path?.Physical?.path) {
    const p = path.Physical.path;
    return p.split("/").pop() || p;
  }

  if (path?.Local?.path) {
    return path.Local.path.split("/").pop() || path.Local.path;
  }

  return "Unknown";
}

// Format SdPath to readable string for subtext
function formatPath(path: any): string {
  if (typeof path === "string") {
    return path.replace(/^\/Users\/[^/]+/, "~");
  }

  if (path?.Physical?.path) {
    const p = path.Physical.path;
    return p.replace(/^\/Users\/[^/]+/, "~");
  }

  if (path?.Local?.path) {
    return path.Local.path.replace(/^\/Users\/[^/]+/, "~");
  }

  return JSON.stringify(path);
}
