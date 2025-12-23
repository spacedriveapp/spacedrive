import {
	Play,
	Pause,
	SpeakerHigh,
	SpeakerSlash,
	ArrowsOut,
	ClosedCaptioning,
	MagnifyingGlassPlus,
	MagnifyingGlassMinus,
	ArrowCounterClockwise,
	Gear,
	Repeat,
} from "@phosphor-icons/react";
import { motion, AnimatePresence } from "framer-motion";
import type { File } from "@sd/ts-client";
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
		(s) => s.kind === "transcript" && s.variant === "srt",
	);

	return (
		<AnimatePresence>
			{showControls && (
				<motion.div
					initial={{ opacity: 0 }}
					animate={{ opacity: 1 }}
					exit={{ opacity: 0 }}
					transition={{ duration: 0.2 }}
					className="absolute bottom-0 z-50 bg-gradient-to-t from-black/80 via-black/40 to-transparent px-4 pb-4 pt-16"
					style={{
						pointerEvents: "auto",
						left: sidebarWidth,
						right: inspectorWidth,
					}}
					onMouseMove={callbacks.onMouseMove}
				>
					{/* Timeline Scrubber Preview */}
					{state.timelineHover && (
						<TimelineScrubber
							file={file}
							hoverPercent={state.timelineHover.percent}
							mouseX={state.timelineHover.mouseX}
							duration={state.duration}
							sidebarWidth={sidebarWidth}
							inspectorWidth={inspectorWidth}
						/>
					)}

					{/* Progress Bar with Thick Hover Area */}
					<div
						className="group mb-3 cursor-pointer relative py-2 -my-2"
						onMouseDown={(e) => {
							callbacks.onSeekingStart();
							callbacks.onSeek(e);
						}}
						onMouseMove={(e) => {
							if (state.seeking) {
								callbacks.onSeek(e);
							} else {
								callbacks.onTimelineHover(e);
							}
						}}
						onMouseEnter={callbacks.onTimelineHover}
						onMouseUp={callbacks.onSeekingEnd}
						onMouseLeave={callbacks.onTimelineLeave}
					>
						<div className="relative h-1 w-full overflow-hidden rounded-full bg-white/20 transition-all group-hover:h-1.5">
							{/* Progress */}
							<div
								className="absolute left-0 top-0 h-full bg-accent transition-all"
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
							onClick={callbacks.onTogglePlay}
							className="rounded-md p-2 text-white transition-colors hover:bg-white/10"
						>
							{state.playing ? (
								<Pause size={20} weight="fill" />
							) : (
								<Play size={20} weight="fill" />
							)}
						</button>

						{/* Loop */}
						<button
							onClick={callbacks.onLoopToggle}
							className={`rounded-md p-2 transition-colors ${
								state.loop
									? "bg-accent/20 text-accent"
									: "text-white hover:bg-white/10"
							}`}
							title="Loop (L)"
						>
							<Repeat size={20} weight="bold" />
						</button>

						{/* Time */}
						<div className="text-sm font-medium text-white">
							{formatTime(state.currentTime)} /{" "}
							{formatTime(state.duration)}
						</div>

						<div className="flex-1" />

						{/* Subtitles Controls */}
						{hasSubs && (
							<div className="flex items-center gap-1">
								<button
									onClick={callbacks.onSubtitlesToggle}
									className={`rounded-md p-2 transition-colors ${
										state.subtitlesEnabled
											? "bg-accent/20 text-accent"
											: "text-white hover:bg-white/10"
									}`}
									title="Toggle subtitles"
								>
									<ClosedCaptioning size={20} weight="fill" />
								</button>
								{state.subtitlesEnabled && (
									<button
										onClick={
											callbacks.onSubtitleSettingsToggle
										}
										className="rounded-md p-2 text-white transition-colors hover:bg-white/10"
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
								onClick={callbacks.onZoomOut}
								disabled={state.zoom <= 1}
								className="rounded-md p-2 text-white transition-colors hover:bg-white/10 disabled:opacity-30"
								title="Zoom out (-)"
							>
								<MagnifyingGlassMinus size={18} weight="bold" />
							</button>
							<button
								onClick={callbacks.onZoomIn}
								disabled={state.zoom >= 5}
								className="rounded-md p-2 text-white transition-colors hover:bg-white/10 disabled:opacity-30"
								title="Zoom in (+)"
							>
								<MagnifyingGlassPlus size={18} weight="bold" />
							</button>
							{state.zoom > 1 && (
								<button
									onClick={callbacks.onZoomReset}
									className="rounded-md p-2 text-white transition-colors hover:bg-white/10"
									title="Reset zoom (0)"
								>
									<ArrowCounterClockwise
										size={18}
										weight="bold"
									/>
								</button>
							)}
						</div>

						{/* Volume */}
						<div className="group flex items-center gap-2">
							<button
								onClick={callbacks.onMuteToggle}
								className="rounded-md p-2 text-white transition-colors hover:bg-white/10"
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
									type="range"
									min="0"
									max="1"
									step="0.01"
									value={state.volume}
									onChange={(e) =>
										callbacks.onVolumeChange(
											parseFloat(e.target.value),
										)
									}
									className="h-1 w-full cursor-pointer appearance-none rounded-full bg-white/20 [&::-webkit-slider-thumb]:size-3 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-white"
								/>
							</div>
						</div>

						{/* Fullscreen */}
						<button
							onClick={callbacks.onFullscreenToggle}
							className="rounded-md p-2 text-white transition-colors hover:bg-white/10"
						>
							<ArrowsOut size={20} weight="bold" />
						</button>
					</div>
				</motion.div>
			)}
		</AnimatePresence>
	);
}
