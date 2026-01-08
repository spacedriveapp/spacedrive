import { AnimatePresence, motion } from "framer-motion";
import type { SubtitleSettings } from "./Subtitles";

interface SubtitleSettingsMenuProps {
  isOpen: boolean;
  settings: SubtitleSettings;
  onSettingsChange: (settings: SubtitleSettings) => void;
  onClose: () => void;
}

export function SubtitleSettingsMenu({
  isOpen,
  settings,
  onSettingsChange,
  onClose,
}: SubtitleSettingsMenuProps) {
  return (
    <AnimatePresence>
      {isOpen && (
        <>
          {/* Backdrop */}
          <div className="fixed inset-0 z-10" onClick={onClose} />

          {/* Settings Menu */}
          <motion.div
            animate={{ opacity: 1, y: 0 }}
            className="absolute right-6 bottom-20 z-20 w-72 rounded-lg border border-app-line bg-sidebar-box/95 p-4 shadow-2xl backdrop-blur-xl"
            exit={{ opacity: 0, y: 10 }}
            initial={{ opacity: 0, y: 10 }}
            onClick={(e) => e.stopPropagation()}
            transition={{ duration: 0.15 }}
          >
            <h3 className="mb-4 font-semibold text-ink text-sm">
              Subtitle Settings
            </h3>

            <div className="space-y-4">
              {/* Font Size */}
              <div>
                <label className="mb-2 flex items-center justify-between text-ink-dull text-xs">
                  <span>Font Size</span>
                  <span className="text-ink">
                    {Math.round(settings.fontSize * 100)}%
                  </span>
                </label>
                <input
                  className="h-1.5 w-full cursor-pointer appearance-none rounded-full bg-sidebar-line [&::-webkit-slider-thumb]:size-3.5 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-accent [&::-webkit-slider-thumb]:shadow-lg"
                  max="2.5"
                  min="0.8"
                  onChange={(e) =>
                    onSettingsChange({
                      ...settings,
                      fontSize: Number.parseFloat(e.target.value),
                    })
                  }
                  step="0.1"
                  type="range"
                  value={settings.fontSize}
                />
              </div>

              {/* Background Opacity */}
              <div>
                <label className="mb-2 flex items-center justify-between text-ink-dull text-xs">
                  <span>Background Opacity</span>
                  <span className="text-ink">
                    {Math.round(settings.backgroundOpacity * 100)}%
                  </span>
                </label>
                <input
                  className="h-1.5 w-full cursor-pointer appearance-none rounded-full bg-sidebar-line [&::-webkit-slider-thumb]:size-3.5 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-accent [&::-webkit-slider-thumb]:shadow-lg"
                  max="1"
                  min="0"
                  onChange={(e) =>
                    onSettingsChange({
                      ...settings,
                      backgroundOpacity: Number.parseFloat(e.target.value),
                    })
                  }
                  step="0.1"
                  type="range"
                  value={settings.backgroundOpacity}
                />
              </div>

              {/* Position */}
              <div>
                <label className="mb-2 block text-ink-dull text-xs">
                  Position
                </label>
                <div className="flex gap-2">
                  <button
                    className={`flex-1 rounded-md px-3 py-2 text-sm transition-colors ${
                      settings.position === "bottom"
                        ? "bg-accent text-white"
                        : "bg-sidebar-line/50 text-ink-dull hover:bg-sidebar-line"
                    }`}
                    onClick={() =>
                      onSettingsChange({
                        ...settings,
                        position: "bottom",
                      })
                    }
                  >
                    Bottom
                  </button>
                  <button
                    className={`flex-1 rounded-md px-3 py-2 text-sm transition-colors ${
                      settings.position === "top"
                        ? "bg-accent text-white"
                        : "bg-sidebar-line/50 text-ink-dull hover:bg-sidebar-line"
                    }`}
                    onClick={() =>
                      onSettingsChange({
                        ...settings,
                        position: "top",
                      })
                    }
                  >
                    Top
                  </button>
                </div>
              </div>
            </div>
          </motion.div>
        </>
      )}
    </AnimatePresence>
  );
}
