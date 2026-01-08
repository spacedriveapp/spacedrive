import { useState, useRef, useEffect } from "react";
import { createPortal } from "react-dom";
import { Sliders } from "@phosphor-icons/react";
import { motion, AnimatePresence } from "framer-motion";
import clsx from "clsx";
import { useExplorer } from "./context";
import { TopBarButton } from "@sd/ui";

interface ViewSettingsProps {
  className?: string;
}

export function ViewSettings({ className }: ViewSettingsProps) {
  const [isOpen, setIsOpen] = useState(false);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState({ top: 0, right: 0 });
  const { viewSettings, setViewSettings, viewMode } = useExplorer();

  // Update position when opened
  useEffect(() => {
    if (isOpen && buttonRef.current) {
      const rect = buttonRef.current.getBoundingClientRect();
      setPosition({
        top: rect.bottom + 8, // 8px gap (mt-2)
        right: window.innerWidth - rect.right,
      });
    }
  }, [isOpen]);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (
        panelRef.current &&
        buttonRef.current &&
        !panelRef.current.contains(e.target as Node) &&
        !buttonRef.current.contains(e.target as Node)
      ) {
        setIsOpen(false);
      }
    };

    if (isOpen) {
      document.addEventListener("mousedown", handleClickOutside);
      return () => document.removeEventListener("mousedown", handleClickOutside);
    }
  }, [isOpen]);

  return (
    <>
      <div className={clsx(className)}>
        <TopBarButton
          ref={buttonRef}
          icon={Sliders}
          onClick={() => setIsOpen(!isOpen)}
          active={isOpen}
          title="View Settings"
        />
      </div>

      {isOpen &&
        createPortal(
          <AnimatePresence>
            <motion.div
              ref={panelRef}
              initial={{ opacity: 0, y: -10 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -10 }}
              transition={{ duration: 0.15 }}
              style={{
                position: "fixed",
                top: `${position.top}px`,
                right: `${position.right}px`,
              }}
              className="w-64 bg-app-box border border-app-line rounded-lg shadow-lg p-3 space-y-4 z-50"
            >
            <div className="text-xs font-semibold text-sidebar-ink uppercase tracking-wider">
              View Settings
            </div>

            {/* Column Width (Column View Only) */}
            {viewMode === "column" && (
              <div className="space-y-2">
                <div className="flex items-center justify-between">
                  <label className="text-xs text-sidebar-inkDull">
                    Column Width
                  </label>
                  <span className="text-xs text-sidebar-ink font-medium">
                    {viewSettings.columnWidth}px
                  </span>
                </div>
                <input
                  type="range"
                  min="200"
                  max="400"
                  step="8"
                  value={viewSettings.columnWidth}
                  onChange={(e) =>
                    setViewSettings({ columnWidth: parseInt(e.target.value) })
                  }
                  className="w-full h-1 bg-app-line rounded-lg appearance-none cursor-pointer
                    [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:w-3.5 [&::-webkit-slider-thumb]:h-3.5 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-accent [&::-webkit-slider-thumb]:cursor-pointer
                    [&::-moz-range-thumb]:w-3.5 [&::-moz-range-thumb]:h-3.5 [&::-moz-range-thumb]:rounded-full [&::-moz-range-thumb]:bg-accent [&::-moz-range-thumb]:cursor-pointer [&::-moz-range-thumb]:border-0"
                />
              </div>
            )}

            {/* Grid Size (Grid View and Media View) */}
            {(viewMode === "grid" || viewMode === "media") && (
              <div className="space-y-2">
                <div className="flex items-center justify-between">
                  <label className="text-xs text-sidebar-inkDull">
                    {viewMode === "media" ? "Thumbnail Size" : "Grid Size"}
                  </label>
                  <span className="text-xs text-sidebar-ink font-medium">
                    {viewSettings.gridSize}px
                  </span>
                </div>
                <input
                  type="range"
                  min="80"
                  max="400"
                  step="10"
                  value={viewSettings.gridSize}
                  onChange={(e) =>
                    setViewSettings({ gridSize: parseInt(e.target.value) })
                  }
                  className="w-full h-1 bg-app-line rounded-lg appearance-none cursor-pointer
                    [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:w-3.5 [&::-webkit-slider-thumb]:h-3.5 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-accent [&::-webkit-slider-thumb]:cursor-pointer
                    [&::-moz-range-thumb]:w-3.5 [&::-moz-range-thumb]:h-3.5 [&::-moz-range-thumb]:rounded-full [&::-moz-range-thumb]:bg-accent [&::-moz-range-thumb]:cursor-pointer [&::-moz-range-thumb]:border-0"
                />
              </div>
            )}

            {/* Gap Size (Grid View Only) */}
            {viewMode === "grid" && (
              <div className="space-y-2">
                <div className="flex items-center justify-between">
                  <label className="text-xs text-sidebar-inkDull">Gap Size</label>
                  <span className="text-xs text-sidebar-ink font-medium">
                    {viewSettings.gapSize}px
                  </span>
                </div>
                <input
                  type="range"
                  min="1"
                  max="32"
                  step="1"
                  value={viewSettings.gapSize}
                  onChange={(e) =>
                    setViewSettings({ gapSize: parseInt(e.target.value) })
                  }
                  className="w-full h-1 bg-app-line rounded-lg appearance-none cursor-pointer
                    [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:w-3.5 [&::-webkit-slider-thumb]:h-3.5 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-accent [&::-webkit-slider-thumb]:cursor-pointer
                    [&::-moz-range-thumb]:w-3.5 [&::-moz-range-thumb]:h-3.5 [&::-moz-range-thumb]:rounded-full [&::-moz-range-thumb]:bg-accent [&::-moz-range-thumb]:cursor-pointer [&::-moz-range-thumb]:border-0"
                />
              </div>
            )}

            {/* Show File Size Toggle */}
            <div className="flex items-center justify-between pt-1">
              <label className="text-xs text-sidebar-inkDull">
                Show File Size
              </label>
              <button
                onClick={() =>
                  setViewSettings({ showFileSize: !viewSettings.showFileSize })
                }
                className={clsx(
                  "relative w-9 h-5 rounded-full transition-colors",
                  viewSettings.showFileSize ? "bg-accent" : "bg-app-line",
                )}
              >
                <motion.div
                  className="absolute top-0.5 left-0.5 w-4 h-4 bg-white rounded-full"
                  animate={{
                    x: viewSettings.showFileSize ? 16 : 0,
                  }}
                  transition={{ duration: 0.2 }}
                />
              </button>
            </div>

            {/* Folders First Toggle */}
            <div className="flex items-center justify-between pt-1">
              <label className="text-xs text-sidebar-inkDull">
                Folders First
              </label>
              <button
                onClick={() =>
                  setViewSettings({ foldersFirst: !viewSettings.foldersFirst })
                }
                className={clsx(
                  "relative w-9 h-5 rounded-full transition-colors",
                  viewSettings.foldersFirst ? "bg-accent" : "bg-app-line",
                )}
              >
                <motion.div
                  className="absolute top-0.5 left-0.5 w-4 h-4 bg-white rounded-full"
                  animate={{
                    x: viewSettings.foldersFirst ? 16 : 0,
                  }}
                  transition={{ duration: 0.2 }}
                />
              </button>
            </div>
          </motion.div>
          </AnimatePresence>,
          document.body
        )}
    </>
  );
}