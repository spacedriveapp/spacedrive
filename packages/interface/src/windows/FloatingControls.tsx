import { usePlatform } from "../contexts/PlatformContext";

/**
 * Floating controls overlay - demonstrates Cap-style tiny floating windows
 * Transparent background, rounded, always on top
 */
export function FloatingControls() {
  const platform = usePlatform();

  const handleClose = () => {
    if (platform.closeWindow) {
      platform.closeWindow("floating-controls");
    }
  };

  return (
    <div className="flex h-full w-full items-center justify-center p-2">
      <div
        className="rounded-full border border-white/10 bg-black/80 px-4 py-3 shadow-2xl backdrop-blur-xl"
        style={{ WebkitAppRegion: "drag" } as any}
      >
        <div className="flex items-center gap-3">
          <button
            className="flex h-8 w-8 items-center justify-center rounded-full bg-red-500 transition-colors hover:bg-red-600"
            onClick={() => alert("Stop!")}
            style={{ WebkitAppRegion: "no-drag" } as any}
          >
            <div className="h-3 w-3 rounded-sm bg-white" />
          </button>
          <button
            className="flex h-8 w-8 items-center justify-center rounded-full bg-gray-600 transition-colors hover:bg-gray-500"
            onClick={handleClose}
            style={{ WebkitAppRegion: "no-drag" } as any}
          >
            <span className="text-white text-xl leading-none">×</span>
          </button>
        </div>
      </div>
    </div>
  );
}
