import { ArrowLeft, ArrowRight, X } from "@phosphor-icons/react";
import type { File } from "@sd/ts-client";
import { AnimatePresence, motion } from "framer-motion";
import { useEffect } from "react";
import { useNormalizedQuery } from "../../contexts/SpacedriveContext";
import { ContentRenderer } from "./ContentRenderer";

interface QuickPreviewOverlayProps {
  fileId: string;
  isOpen: boolean;
  onClose: () => void;
  onNext?: () => void;
  onPrevious?: () => void;
  hasPrevious?: boolean;
  hasNext?: boolean;
}

export function QuickPreviewOverlay({
  fileId,
  isOpen,
  onClose,
  onNext,
  onPrevious,
  hasPrevious,
  hasNext,
}: QuickPreviewOverlayProps) {
  const {
    data: file,
    isLoading,
    error,
  } = useNormalizedQuery<{ file_id: string }, File>({
    wireMethod: "query:files.by_id",
    input: { file_id: fileId },
    resourceType: "file",
    resourceId: fileId,
    enabled: !!fileId && isOpen,
  });

  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.code === "Escape" || e.code === "Space") {
        e.preventDefault();
        onClose();
      }
      if (e.code === "ArrowLeft" && hasPrevious && onPrevious) {
        e.preventDefault();
        onPrevious();
      }
      if (e.code === "ArrowRight" && hasNext && onNext) {
        e.preventDefault();
        onNext();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isOpen, onClose, onNext, onPrevious, hasPrevious, hasNext]);

  return (
    <AnimatePresence mode="wait">
      {isOpen && (
        <motion.div
          animate={{ opacity: 1 }}
          className="absolute inset-0 z-50 flex flex-col overflow-hidden rounded-lg bg-app/95 backdrop-blur-xl"
          exit={{ opacity: 0 }}
          initial={{ opacity: 0 }}
          key="overlay"
          transition={{ duration: 0.15 }}
        >
          {isLoading || !file ? (
            <div className="flex h-full items-center justify-center text-ink">
              <div className="animate-pulse">Loading...</div>
            </div>
          ) : error ? (
            <div className="flex h-full items-center justify-center text-red-400">
              <div>
                <div className="mb-2 font-medium text-lg">
                  Error loading file
                </div>
                <div className="text-sm">{error.message}</div>
              </div>
            </div>
          ) : (
            <>
              {/* Header */}
              <div className="flex items-center justify-between border-app-line/50 border-b bg-app-box/40 px-4 py-2">
                <div className="flex flex-1 items-center gap-3">
                  {/* Navigation Arrows */}
                  {(hasPrevious || hasNext) && (
                    <>
                      <div className="flex items-center gap-1">
                        <button
                          className="rounded-md p-1.5 text-ink-dull transition-colors hover:bg-app-hover hover:text-ink disabled:opacity-30"
                          disabled={!hasPrevious}
                          onClick={onPrevious}
                        >
                          <ArrowLeft size={16} weight="bold" />
                        </button>
                        <button
                          className="rounded-md p-1.5 text-ink-dull transition-colors hover:bg-app-hover hover:text-ink disabled:opacity-30"
                          disabled={!hasNext}
                          onClick={onNext}
                        >
                          <ArrowRight size={16} weight="bold" />
                        </button>
                      </div>
                      <div className="h-4 w-px bg-app-line/50" />
                    </>
                  )}
                  <div className="truncate font-medium text-ink text-sm">
                    {file.name}
                  </div>
                </div>

                <button
                  className="rounded-md p-1.5 text-ink-dull transition-colors hover:bg-app-hover hover:text-ink"
                  onClick={onClose}
                >
                  <X size={16} weight="bold" />
                </button>
              </div>

              {/* Content Area - full width, no inspector */}
              <div className="flex-1 overflow-hidden">
                <ContentRenderer file={file} />
              </div>

              {/* Footer with keyboard hints */}
              <div className="border-app-line/50 border-t bg-app-box/40 px-4 py-1.5">
                <div className="text-center text-ink-dull text-xs">
                  <span className="text-ink">ESC</span> or{" "}
                  <span className="text-ink">Space</span> to close
                  {(hasPrevious || hasNext) && (
                    <>
                      {" · "}
                      <span className="text-ink">←</span> /{" "}
                      <span className="text-ink">→</span> to navigate
                    </>
                  )}
                </div>
              </div>
            </>
          )}
        </motion.div>
      )}
    </AnimatePresence>
  );
}
