import { Sliders } from "@phosphor-icons/react";
import { TopBarButton } from "@sd/ui";
import clsx from "clsx";
import { AnimatePresence, motion } from "framer-motion";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useExplorer } from "./context";

interface ViewSettingsPanelProps {
  viewSettings: any;
  setViewSettings: (settings: any) => void;
  viewMode: string;
}

export function ViewSettingsPanel({
  viewSettings,
  setViewSettings,
  viewMode,
}: ViewSettingsPanelProps) {
  return (
    <div className="w-64 space-y-4 rounded-lg border border-app-line bg-app-box p-3 shadow-lg">
      <div className="font-semibold text-sidebar-ink text-xs uppercase tracking-wider">
        View Settings
      </div>

      {/* Column Width (Column View Only) */}
      {viewMode === "column" && (
        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <label className="text-sidebar-inkDull text-xs">Column Width</label>
            <span className="font-medium text-sidebar-ink text-xs">
              {viewSettings.columnWidth}px
            </span>
          </div>
          <input
            className="h-1 w-full cursor-pointer appearance-none rounded-lg bg-app-line [&::-moz-range-thumb]:h-3.5 [&::-moz-range-thumb]:w-3.5 [&::-moz-range-thumb]:cursor-pointer [&::-moz-range-thumb]:rounded-full [&::-moz-range-thumb]:border-0 [&::-moz-range-thumb]:bg-accent [&::-webkit-slider-thumb]:h-3.5 [&::-webkit-slider-thumb]:w-3.5 [&::-webkit-slider-thumb]:cursor-pointer [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-accent"
            max="400"
            min="200"
            onChange={(e) =>
              setViewSettings({ columnWidth: Number.parseInt(e.target.value) })
            }
            step="8"
            type="range"
            value={viewSettings.columnWidth}
          />
        </div>
      )}

      {/* Grid Size (Grid View and Media View) */}
      {(viewMode === "grid" || viewMode === "media") && (
        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <label className="text-sidebar-inkDull text-xs">
              {viewMode === "media" ? "Thumbnail Size" : "Grid Size"}
            </label>
            <span className="font-medium text-sidebar-ink text-xs">
              {viewSettings.gridSize}px
            </span>
          </div>
          <input
            className="h-1 w-full cursor-pointer appearance-none rounded-lg bg-app-line [&::-moz-range-thumb]:h-3.5 [&::-moz-range-thumb]:w-3.5 [&::-moz-range-thumb]:cursor-pointer [&::-moz-range-thumb]:rounded-full [&::-moz-range-thumb]:border-0 [&::-moz-range-thumb]:bg-accent [&::-webkit-slider-thumb]:h-3.5 [&::-webkit-slider-thumb]:w-3.5 [&::-webkit-slider-thumb]:cursor-pointer [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-accent"
            max="400"
            min="80"
            onChange={(e) =>
              setViewSettings({ gridSize: Number.parseInt(e.target.value) })
            }
            step="10"
            type="range"
            value={viewSettings.gridSize}
          />
        </div>
      )}

      {/* Gap Size (Grid View Only) */}
      {viewMode === "grid" && (
        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <label className="text-sidebar-inkDull text-xs">Gap Size</label>
            <span className="font-medium text-sidebar-ink text-xs">
              {viewSettings.gapSize}px
            </span>
          </div>
          <input
            className="h-1 w-full cursor-pointer appearance-none rounded-lg bg-app-line [&::-moz-range-thumb]:h-3.5 [&::-moz-range-thumb]:w-3.5 [&::-moz-range-thumb]:cursor-pointer [&::-moz-range-thumb]:rounded-full [&::-moz-range-thumb]:border-0 [&::-moz-range-thumb]:bg-accent [&::-webkit-slider-thumb]:h-3.5 [&::-webkit-slider-thumb]:w-3.5 [&::-webkit-slider-thumb]:cursor-pointer [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-accent"
            max="32"
            min="1"
            onChange={(e) =>
              setViewSettings({ gapSize: Number.parseInt(e.target.value) })
            }
            step="1"
            type="range"
            value={viewSettings.gapSize}
          />
        </div>
      )}

      {/* Show File Size Toggle */}
      <div className="flex items-center justify-between pt-1">
        <label className="text-sidebar-inkDull text-xs">Show File Size</label>
        <button
          className={clsx(
            "relative h-5 w-9 rounded-full transition-colors",
            viewSettings.showFileSize ? "bg-accent" : "bg-app-line"
          )}
          onClick={() =>
            setViewSettings({ showFileSize: !viewSettings.showFileSize })
          }
        >
          <motion.div
            animate={{
              x: viewSettings.showFileSize ? 16 : 0,
            }}
            className="absolute top-0.5 left-0.5 h-4 w-4 rounded-full bg-white"
            transition={{ duration: 0.2 }}
          />
        </button>
      </div>

      {/* Folders First Toggle */}
      <div className="flex items-center justify-between pt-1">
        <label className="text-sidebar-inkDull text-xs">Folders First</label>
        <button
          className={clsx(
            "relative h-5 w-9 rounded-full transition-colors",
            viewSettings.foldersFirst ? "bg-accent" : "bg-app-line"
          )}
          onClick={() =>
            setViewSettings({ foldersFirst: !viewSettings.foldersFirst })
          }
        >
          <motion.div
            animate={{
              x: viewSettings.foldersFirst ? 16 : 0,
            }}
            className="absolute top-0.5 left-0.5 h-4 w-4 rounded-full bg-white"
            transition={{ duration: 0.2 }}
          />
        </button>
      </div>
    </div>
  );
}

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
      return () =>
        document.removeEventListener("mousedown", handleClickOutside);
    }
  }, [isOpen]);

  return (
    <>
      <div className={clsx(className)}>
        <TopBarButton
          active={isOpen}
          icon={Sliders}
          onClick={() => setIsOpen(!isOpen)}
          ref={buttonRef}
          title="View Settings"
        />
      </div>

      {isOpen &&
        createPortal(
          <AnimatePresence>
            <motion.div
              animate={{ opacity: 1, y: 0 }}
              className="z-50"
              exit={{ opacity: 0, y: -10 }}
              initial={{ opacity: 0, y: -10 }}
              ref={panelRef}
              style={{
                position: "fixed",
                top: `${position.top}px`,
                right: `${position.right}px`,
              }}
              transition={{ duration: 0.15 }}
            >
              <ViewSettingsPanel
                setViewSettings={setViewSettings}
                viewMode={viewMode}
                viewSettings={viewSettings}
              />
            </motion.div>
          </AnimatePresence>,
          document.body
        )}
    </>
  );
}
