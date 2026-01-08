import { ArrowLeft, ArrowRight, X } from "@phosphor-icons/react";
import { AnimatePresence, motion } from "framer-motion";
import { useEffect, useMemo, useState } from "react";
import { createPortal } from "react-dom";
import { useExplorer } from "../../routes/explorer/context";
import { getContentKind } from "../../routes/explorer/utils";
import { TopBarItem, TopBarPortal } from "../../TopBar";
import { ContentRenderer } from "./ContentRenderer";
import {
  VideoControls,
  type VideoControlsCallbacks,
  type VideoControlsState,
} from "./VideoControls";

interface QuickPreviewFullscreenProps {
  fileId: string;
  isOpen: boolean;
  onClose: () => void;
  onNext?: () => void;
  onPrevious?: () => void;
  hasPrevious?: boolean;
  hasNext?: boolean;
  sidebarWidth?: number;
  inspectorWidth?: number;
}

const PREVIEW_LAYER_ID = "quick-preview-layer";

export function QuickPreviewFullscreen({
  fileId,
  isOpen,
  onClose,
  onNext,
  onPrevious,
  hasPrevious,
  hasNext,
  sidebarWidth = 0,
  inspectorWidth = 0,
}: QuickPreviewFullscreenProps) {
  const [portalTarget, setPortalTarget] = useState<HTMLElement | null>(null);
  const [isZoomed, setIsZoomed] = useState(false);
  const [videoControlsState, setVideoControlsState] =
    useState<VideoControlsState | null>(null);
  const [showVideoControls, setShowVideoControls] = useState(false);
  const [videoCallbacks, setVideoCallbacks] =
    useState<VideoControlsCallbacks | null>(null);
  const { currentFiles } = useExplorer();

  // Reset zoom when file changes
  useEffect(() => {
    setIsZoomed(false);
  }, [fileId]);

  // Get file directly from currentFiles - instant, no network request
  const file = useMemo(
    () => currentFiles.find((f) => f.id === fileId) ?? null,
    [currentFiles, fileId]
  );

  // No query needed - files are already loaded by the explorer views
  const isLoading = false;
  const error = null;

  // Find portal target on mount
  useEffect(() => {
    const target = document.getElementById(PREVIEW_LAYER_ID);
    setPortalTarget(target);
  }, []);

  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      // Only handle close events - let Explorer handle navigation
      if (e.code === "Escape" || e.code === "Space") {
        e.preventDefault();
        e.stopImmediatePropagation();
        onClose();
      }
    };

    window.addEventListener("keydown", handleKeyDown, { capture: true });
    return () =>
      window.removeEventListener("keydown", handleKeyDown, {
        capture: true,
      });
  }, [isOpen, onClose]);

  // Get background style based on content type
  const getBackgroundClass = () => {
    if (!file) return "bg-black/90";

    switch (getContentKind(file)) {
      case "video":
        return "bg-black";
      case "audio":
        return "audio-gradient";
      case "image":
        return "bg-black/95";
      default:
        return "bg-black/90";
    }
  };

  // Memoize TopBarItem children to prevent infinite re-renders
  const navigationButtons = useMemo(
    () => (
      <div className="flex items-center gap-2">
        <button
          className="rounded-md p-1.5 text-white/70 transition-colors hover:bg-white/10 hover:text-white disabled:opacity-30"
          disabled={!hasPrevious}
          onClick={onPrevious}
        >
          <ArrowLeft size={16} weight="bold" />
        </button>
        <button
          className="rounded-md p-1.5 text-white/70 transition-colors hover:bg-white/10 hover:text-white disabled:opacity-30"
          disabled={!hasNext}
          onClick={onNext}
        >
          <ArrowRight size={16} weight="bold" />
        </button>
        <div className="mx-1 h-4 w-px bg-white/20" />
      </div>
    ),
    [onPrevious, onNext, hasPrevious, hasNext]
  );

  const filenameDisplay = useMemo(
    () => (
      <div className="truncate font-medium text-sm text-white/90">
        {file?.name}
      </div>
    ),
    [file?.name]
  );

  const closeButton = useMemo(
    () => (
      <button
        className="rounded-md p-1.5 text-white/70 transition-colors hover:bg-white/10 hover:text-white"
        onClick={onClose}
      >
        <X size={16} weight="bold" />
      </button>
    ),
    [onClose]
  );

  if (!portalTarget) return null;

  const content = (
    <AnimatePresence mode="wait">
      {isOpen && (
        <motion.div
          animate={{ opacity: 1 }}
          className={`absolute inset-0 flex flex-col ${getBackgroundClass()}`}
          exit={{ opacity: 0 }}
          initial={{ opacity: 0 }}
          key="fullscreen-preview"
          transition={{ duration: 0.2 }}
        >
          {!file && isLoading ? (
            <div className="flex h-full items-center justify-center text-ink">
              <div className="animate-pulse">Loading...</div>
            </div>
          ) : !file && error ? (
            <div className="flex h-full items-center justify-center text-red-400">
              <div>
                <div className="mb-2 font-medium text-lg">
                  Error loading file
                </div>
                <div className="text-sm">{error.message}</div>
              </div>
            </div>
          ) : file ? (
            <>
              {/* TopBar content via portal */}
              <TopBarPortal
                center={
                  <TopBarItem
                    id="preview-filename"
                    label="File Name"
                    priority="high"
                  >
                    {filenameDisplay}
                  </TopBarItem>
                }
                left={
                  <>
                    {(hasPrevious || hasNext) && (
                      <TopBarItem
                        id="preview-navigation"
                        label="Navigation"
                        priority="high"
                      >
                        {navigationButtons}
                      </TopBarItem>
                    )}
                  </>
                }
                right={
                  <TopBarItem
                    id="preview-close"
                    label="Close"
                    onClick={onClose}
                    priority="high"
                  >
                    {closeButton}
                  </TopBarItem>
                }
              />

              {/* Content Area - padded to fit between sidebar/inspector, expands on zoom */}
              <div
                className={`flex-1 pt-14 pb-10 ${isZoomed ? "overflow-visible" : "overflow-hidden"}`}
                style={{
                  paddingLeft: isZoomed ? 0 : sidebarWidth,
                  paddingRight: isZoomed ? 0 : inspectorWidth,
                  transition: "padding 0.3s ease-out",
                }}
              >
                <ContentRenderer
                  file={file}
                  getVideoCallbacks={setVideoCallbacks}
                  onShowVideoControlsChange={setShowVideoControls}
                  onVideoControlsStateChange={setVideoControlsState}
                  onZoomChange={setIsZoomed}
                />
              </div>

              {/* Video Controls Overlay - fixed position, always uses sidebar/inspector padding */}
              {videoControlsState &&
                videoCallbacks &&
                getContentKind(file) === "video" && (
                  <div
                    className="absolute inset-0"
                    style={{
                      paddingTop: "56px", // TopBar height
                      paddingBottom: "40px", // Footer height
                      pointerEvents: "none", // Let clicks through except on controls themselves
                    }}
                  >
                    <VideoControls
                      callbacks={videoCallbacks}
                      file={file}
                      inspectorWidth={inspectorWidth}
                      showControls={showVideoControls}
                      sidebarWidth={sidebarWidth}
                      state={videoControlsState}
                    />
                  </div>
                )}

              {/* Footer with keyboard hints */}
              <div className="absolute right-0 bottom-0 left-0 z-10 px-6 py-3">
                <div className="text-center text-white/50 text-xs">
                  <span className="text-white/70">ESC</span> or{" "}
                  <span className="text-white/70">Space</span> to close
                  {(hasPrevious || hasNext) && (
                    <>
                      {" · "}
                      <span className="text-white/70">←</span> /{" "}
                      <span className="text-white/70">→</span> to navigate
                    </>
                  )}
                </div>
              </div>
            </>
          ) : (
            <div className="flex h-full items-center justify-center text-ink-dull">
              <div>File not found</div>
            </div>
          )}
        </motion.div>
      )}
    </AnimatePresence>
  );

  return createPortal(content, portalTarget);
}

export { PREVIEW_LAYER_ID };
