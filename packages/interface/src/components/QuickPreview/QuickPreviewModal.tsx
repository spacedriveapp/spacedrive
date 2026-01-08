import { ArrowLeft, ArrowRight, X } from "@phosphor-icons/react";
import { AnimatePresence, motion } from "framer-motion";
import { useEffect } from "react";
import { useLibraryQuery } from "../../contexts/SpacedriveContext";
import { Inspector } from "../Inspector/Inspector";
import { ContentRenderer } from "./ContentRenderer";

interface QuickPreviewModalProps {
  fileId: string;
  isOpen: boolean;
  onClose: () => void;
  onNext?: () => void;
  onPrevious?: () => void;
  hasPrevious?: boolean;
  hasNext?: boolean;
}

export function QuickPreviewModal({
  fileId,
  isOpen,
  onClose,
  onNext,
  onPrevious,
  hasPrevious,
  hasNext,
}: QuickPreviewModalProps) {
  const {
    data: file,
    isLoading,
    error,
  } = useLibraryQuery(
    {
      type: "files.by_id",
      input: { file_id: fileId },
    },
    {
      enabled: !!fileId && isOpen,
    }
  );

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
        <>
          {/* Backdrop */}
          <motion.div
            animate={{ opacity: 1 }}
            className="fixed inset-0 z-[9999] bg-black/80 backdrop-blur-sm"
            exit={{ opacity: 0 }}
            initial={{ opacity: 0 }}
            key="backdrop"
            onClick={onClose}
            transition={{ duration: 0.15 }}
          />

          {/* Modal - key stays constant so it doesn't remount on file change */}
          <motion.div
            animate={{ opacity: 1, scale: 1 }}
            className="fixed inset-8 z-[9999] flex flex-col overflow-hidden rounded-2xl border border-app-line bg-app shadow-2xl"
            exit={{ opacity: 0, scale: 0.95 }}
            initial={{ opacity: 0, scale: 0.95 }}
            key="modal"
            onClick={(e) => e.stopPropagation()}
            transition={{ duration: 0.2, ease: [0.25, 1, 0.5, 1] }}
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
                <div className="flex items-center justify-between border-app-line border-b bg-app-box/40 px-4 py-3 backdrop-blur-xl">
                  <div className="flex flex-1 items-center gap-3">
                    {/* Navigation Arrows */}
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

                    <div className="h-4 w-px bg-app-line" />

                    <div className="truncate font-medium text-sm">
                      {file.name}
                    </div>
                  </div>

                  <button
                    className="rounded-md p-1 text-ink-dull transition-colors hover:bg-app-hover hover:text-ink"
                    onClick={onClose}
                  >
                    <X size={16} weight="bold" />
                  </button>
                </div>

                {/* Content Area */}
                <div className="flex flex-1 overflow-hidden">
                  {/* File Content */}
                  <div className="flex-1 bg-app-box/30">
                    <ContentRenderer file={file} />
                  </div>

                  {/* Inspector Sidebar */}
                  <div className="w-[280px] min-w-[280px] overflow-hidden border-app-line border-l bg-app">
                    <Inspector
                      showPopOutButton={false}
                      variant={{ type: "file", file }}
                    />
                  </div>
                </div>

                {/* Footer with keyboard hints */}
                <div className="border-app-line border-t bg-app-box/30 px-4 py-2">
                  <div className="text-center text-ink-dull text-xs">
                    <span className="text-ink">ESC</span> or{" "}
                    <span className="text-ink">Space</span> to close
                    {(hasPrevious || hasNext) && (
                      <>
                        {" • "}
                        <span className="text-ink">←</span> /{" "}
                        <span className="text-ink">→</span> to navigate
                      </>
                    )}
                  </div>
                </div>
              </>
            )}
          </motion.div>
        </>
      )}
    </AnimatePresence>
  );
}
