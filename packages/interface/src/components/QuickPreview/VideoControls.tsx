import {
  ArrowCounterClockwise,
  ArrowsOut,
  ClosedCaptioning,
  Gear,
  MagnifyingGlassMinus,
  MagnifyingGlassPlus,
  Pause,
  Play,
  Repeat,
  SpeakerHigh,
  SpeakerSlash,
} from "@phosphor-icons/react";
import type { File } from "@sd/ts-client";
import { AnimatePresence, motion } from "framer-motion";
import { TimelineScrubber } from "./TimelineScrubber";

export interface VideoControlsState {
  playing: boolean;
  currentTime: number;
  duration: number;
  volume: number;
  muted: boolean;
  loop: boolean;
  zoom: number;
  subtitlesEnabled: boolean;
  showSubtitleSettings: boolean;
  seeking: boolean;
  timelineHover: { percent: number; mouseX: number } | null;
}

export interface VideoControlsCallbacks {
  onTogglePlay: () => void;
  onSeek: (e: React.MouseEvent<HTMLDivElement>) => void;
  onTimelineHover: (e: React.MouseEvent<HTMLDivElement>) => void;
  onTimelineLeave: () => void;
  onSeekingStart: () => void;
  onSeekingEnd: () => void;
  onVolumeChange: (volume: number) => void;
  onMuteToggle: () => void;
  onLoopToggle: () => void;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onZoomReset: () => void;
  onSubtitlesToggle: () => void;
  onSubtitleSettingsToggle: () => void;
  onFullscreenToggle: () => void;
  onMouseMove: () => void;
}

interface VideoControlsProps {
  file: File;
  state: VideoControlsState;
  callbacks: VideoControlsCallbacks;
  showControls: boolean;
  sidebarWidth?: number;
  inspectorWidth?: number;
}

function formatTime(seconds: number): string {
  const hours = Math.floor(seconds / 3600);
  const mins = Math.floor((seconds % 3600) / 60);
  const secs = Math.floor(seconds % 60);

  if (hours > 0) {
    return `${hours}:${mins.toString().padStart(2, "0")}:${secs.toString().padStart(2, "0")}`;
  }
  return `${mins}:${secs.toString().padStart(2, "0")}`;
}

export function VideoControls({
  file,
  state,
  callbacks,
  showControls,
  sidebarWidth = 0,
  inspectorWidth = 0,
}: VideoControlsProps) {
  const hasSubs = file.sidecars?.some(
    (s) => s.kind === "transcript" && s.variant === "srt"
  );

  return (
    <AnimatePresence>
      {showControls && (
        <motion.div
          animate={{ opacity: 1 }}
          className="absolute bottom-0 z-50 bg-gradient-to-t from-black/80 via-black/40 to-transparent px-4 pt-16 pb-4"
          exit={{ opacity: 0 }}
          initial={{ opacity: 0 }}
          onMouseMove={callbacks.onMouseMove}
          style={{
            pointerEvents: "auto",
            left: sidebarWidth,
            right: inspectorWidth,
          }}
          transition={{ duration: 0.2 }}
        >
          {/* Timeline Scrubber Preview */}
          {state.timelineHover && (
            <TimelineScrubber
              duration={state.duration}
              file={file}
              hoverPercent={state.timelineHover.percent}
              inspectorWidth={inspectorWidth}
              mouseX={state.timelineHover.mouseX}
              sidebarWidth={sidebarWidth}
            />
          )}

          {/* Progress Bar with Thick Hover Area */}
          <div
            className="group relative -my-2 mb-3 cursor-pointer py-2"
            onMouseDown={(e) => {
              callbacks.onSeekingStart();
              callbacks.onSeek(e);
            }}
            onMouseEnter={callbacks.onTimelineHover}
            onMouseLeave={callbacks.onTimelineLeave}
            onMouseMove={(e) => {
              if (state.seeking) {
                callbacks.onSeek(e);
              } else {
                callbacks.onTimelineHover(e);
              }
            }}
            onMouseUp={callbacks.onSeekingEnd}
          >
            <div className="relative h-1 w-full overflow-hidden rounded-full bg-white/20 transition-all group-hover:h-1.5">
              {/* Progress */}
              <div
                className="absolute top-0 left-0 h-full bg-accent transition-all"
                style={{
                  width: `${(state.currentTime / state.duration) * 100}%`,
                }}
              />

              {/* Scrubber */}
              <div
                className="absolute top-1/2 -translate-y-1/2 transition-all"
                style={{
                  left: `${(state.currentTime / state.duration) * 100}%`,
                }}
              >
                <div className="size-3 -translate-x-1/2 rounded-full bg-accent opacity-0 shadow-lg transition-opacity group-hover:opacity-100" />
              </div>
            </div>
          </div>

          {/* Controls Bar */}
          <div className="flex items-center gap-3">
            {/* Play/Pause */}
            <button
              className="rounded-md p-2 text-white transition-colors hover:bg-white/10"
              onClick={callbacks.onTogglePlay}
            >
              {state.playing ? (
                <Pause size={20} weight="fill" />
              ) : (
                <Play size={20} weight="fill" />
              )}
            </button>

            {/* Loop */}
            <button
              className={`rounded-md p-2 transition-colors ${
                state.loop
                  ? "bg-accent/20 text-accent"
                  : "text-white hover:bg-white/10"
              }`}
              onClick={callbacks.onLoopToggle}
              title="Loop (L)"
            >
              <Repeat size={20} weight="bold" />
            </button>

            {/* Time */}
            <div className="font-medium text-sm text-white">
              {formatTime(state.currentTime)} / {formatTime(state.duration)}
            </div>

            <div className="flex-1" />

            {/* Subtitles Controls */}
            {hasSubs && (
              <div className="flex items-center gap-1">
                <button
                  className={`rounded-md p-2 transition-colors ${
                    state.subtitlesEnabled
                      ? "bg-accent/20 text-accent"
                      : "text-white hover:bg-white/10"
                  }`}
                  onClick={callbacks.onSubtitlesToggle}
                  title="Toggle subtitles"
                >
                  <ClosedCaptioning size={20} weight="fill" />
                </button>
                {state.subtitlesEnabled && (
                  <button
                    className="rounded-md p-2 text-white transition-colors hover:bg-white/10"
                    onClick={callbacks.onSubtitleSettingsToggle}
                    title="Subtitle settings"
                  >
                    <Gear size={20} weight="fill" />
                  </button>
                )}
              </div>
            )}

            {/* Zoom Controls */}
            <div className="flex items-center gap-1">
              <button
                className="rounded-md p-2 text-white transition-colors hover:bg-white/10 disabled:opacity-30"
                disabled={state.zoom <= 1}
                onClick={callbacks.onZoomOut}
                title="Zoom out (-)"
              >
                <MagnifyingGlassMinus size={18} weight="bold" />
              </button>
              <button
                className="rounded-md p-2 text-white transition-colors hover:bg-white/10 disabled:opacity-30"
                disabled={state.zoom >= 5}
                onClick={callbacks.onZoomIn}
                title="Zoom in (+)"
              >
                <MagnifyingGlassPlus size={18} weight="bold" />
              </button>
              {state.zoom > 1 && (
                <button
                  className="rounded-md p-2 text-white transition-colors hover:bg-white/10"
                  onClick={callbacks.onZoomReset}
                  title="Reset zoom (0)"
                >
                  <ArrowCounterClockwise size={18} weight="bold" />
                </button>
              )}
            </div>

            {/* Volume */}
            <div className="group flex items-center gap-2">
              <button
                className="rounded-md p-2 text-white transition-colors hover:bg-white/10"
                onClick={callbacks.onMuteToggle}
              >
                {state.muted || state.volume === 0 ? (
                  <SpeakerSlash size={20} weight="fill" />
                ) : (
                  <SpeakerHigh size={20} weight="fill" />
                )}
              </button>

              {/* Volume Slider */}
              <div className="w-0 overflow-hidden transition-all group-hover:w-20">
                <input
                  className="h-1 w-full cursor-pointer appearance-none rounded-full bg-white/20 [&::-webkit-slider-thumb]:size-3 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-white"
                  max="1"
                  min="0"
                  onChange={(e) =>
                    callbacks.onVolumeChange(Number.parseFloat(e.target.value))
                  }
                  step="0.01"
                  type="range"
                  value={state.volume}
                />
              </div>
            </div>

            {/* Fullscreen */}
            <button
              className="rounded-md p-2 text-white transition-colors hover:bg-white/10"
              onClick={callbacks.onFullscreenToggle}
            >
              <ArrowsOut size={20} weight="bold" />
            </button>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
