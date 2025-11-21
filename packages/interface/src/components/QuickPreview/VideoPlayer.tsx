import { useState, useRef, useEffect } from 'react';
import { Play, Pause, SpeakerHigh, SpeakerSlash, ArrowsOut, ClosedCaptioning, MagnifyingGlassPlus, MagnifyingGlassMinus, ArrowCounterClockwise, Gear } from '@phosphor-icons/react';
import { motion, AnimatePresence } from 'framer-motion';
import type { File } from '@sd/ts-client';
import { Subtitles, type SubtitleSettings } from './Subtitles';
import { SubtitleSettingsMenu } from './SubtitleSettingsMenu';
import { useZoomPan } from './useZoomPan';
import { TimelineScrubber } from './TimelineScrubber';

interface VideoPlayerProps {
	src: string;
	file: File;
}

function formatTime(seconds: number): string {
	const hours = Math.floor(seconds / 3600);
	const mins = Math.floor((seconds % 3600) / 60);
	const secs = Math.floor(seconds % 60);

	if (hours > 0) {
		return `${hours}:${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
	}
	return `${mins}:${secs.toString().padStart(2, '0')}`;
}

export function VideoPlayer({ src, file }: VideoPlayerProps) {
	const videoRef = useRef<HTMLVideoElement>(null);
	const containerRef = useRef<HTMLDivElement>(null);
	const videoContainerRef = useRef<HTMLDivElement>(null);
	const [playing, setPlaying] = useState(false);
	const [currentTime, setCurrentTime] = useState(0);
	const [duration, setDuration] = useState(0);
	const [volume, setVolume] = useState(1);
	const [muted, setMuted] = useState(false);
	const [showControls, setShowControls] = useState(true);
	const [seeking, setSeeking] = useState(false);
	const [subtitlesEnabled, setSubtitlesEnabled] = useState(true);
	const [showSubtitleSettings, setShowSubtitleSettings] = useState(false);
	const [subtitleSettings, setSubtitleSettings] = useState<SubtitleSettings>({
		fontSize: 1.5,
		position: 'bottom',
		backgroundOpacity: 0.9
	});
	const [timelineHover, setTimelineHover] = useState<{ percent: number; mouseX: number } | null>(null);
	const hideControlsTimeout = useRef<NodeJS.Timeout>();
	const { zoom, zoomIn, zoomOut, reset, transform } = useZoomPan(videoContainerRef);

	// Show controls on mouse move, hide after 3s of inactivity
	const handleMouseMove = () => {
		setShowControls(true);
		if (hideControlsTimeout.current) {
			clearTimeout(hideControlsTimeout.current);
		}
		if (playing) {
			hideControlsTimeout.current = setTimeout(() => {
				setShowControls(false);
			}, 3000);
		}
	};

	// Keyboard shortcuts
	useEffect(() => {
		const handleKeyDown = (e: KeyboardEvent) => {
			if (!videoRef.current) return;

			switch (e.code) {
				case 'Space':
					e.preventDefault();
					togglePlay();
					break;
				case 'ArrowLeft':
					e.preventDefault();
					videoRef.current.currentTime = Math.max(0, videoRef.current.currentTime - 5);
					break;
				case 'ArrowRight':
					e.preventDefault();
					videoRef.current.currentTime = Math.min(
						duration,
						videoRef.current.currentTime + 5
					);
					break;
				case 'ArrowUp':
					e.preventDefault();
					setVolume((v) => Math.min(1, v + 0.1));
					break;
				case 'ArrowDown':
					e.preventDefault();
					setVolume((v) => Math.max(0, v - 0.1));
					break;
				case 'KeyM':
					e.preventDefault();
					setMuted((m) => !m);
					break;
				case 'KeyF':
					e.preventDefault();
					toggleFullscreen();
					break;
				case 'KeyC':
					e.preventDefault();
					setSubtitlesEnabled((s) => !s);
					break;
			}
		};

		window.addEventListener('keydown', handleKeyDown);
		return () => window.removeEventListener('keydown', handleKeyDown);
	}, [duration, playing]);

	// Sync video element state
	useEffect(() => {
		if (!videoRef.current) return;
		videoRef.current.volume = volume;
	}, [volume]);

	useEffect(() => {
		if (!videoRef.current) return;
		videoRef.current.muted = muted;
	}, [muted]);

	const togglePlay = () => {
		if (!videoRef.current) return;
		if (playing) {
			videoRef.current.pause();
		} else {
			videoRef.current.play();
		}
	};

	const handleSeek = (e: React.MouseEvent<HTMLDivElement>) => {
		if (!videoRef.current) return;
		const rect = e.currentTarget.getBoundingClientRect();
		const percent = (e.clientX - rect.left) / rect.width;
		videoRef.current.currentTime = percent * duration;
	};

	const handleTimelineHover = (e: React.MouseEvent<HTMLDivElement>) => {
		const rect = e.currentTarget.getBoundingClientRect();
		const percent = (e.clientX - rect.left) / rect.width;
		setTimelineHover({ percent, mouseX: e.clientX });
	};

	const toggleFullscreen = () => {
		if (!containerRef.current) return;
		if (document.fullscreenElement) {
			document.exitFullscreen();
		} else {
			containerRef.current.requestFullscreen();
		}
	};

	const hasSubs = file.sidecars?.some(s => s.kind === 'transcript' && s.variant === 'srt');

	return (
		<div
			ref={containerRef}
			className="relative flex h-full w-full items-center justify-center bg-black"
			onMouseMove={handleMouseMove}
			onMouseLeave={() => playing && setShowControls(false)}
		>
			{/* Zoom level indicator */}
			{zoom > 1 && (
				<div className="absolute top-4 left-4 z-20 rounded-lg bg-black/80 px-3 py-1.5 text-sm font-medium text-white backdrop-blur-xl">
					{Math.round(zoom * 100)}%
				</div>
			)}

			{/* Video container with zoom/pan */}
			<div
				ref={videoContainerRef}
				className="relative flex h-full w-full items-center justify-center overflow-hidden"
			>
				<div style={transform} className="flex items-center justify-center">
					<video
						ref={videoRef}
						src={src}
						autoPlay
						playsInline
						className="max-h-screen max-w-screen"
						onPlay={() => setPlaying(true)}
						onPause={() => setPlaying(false)}
						onTimeUpdate={(e) => !seeking && setCurrentTime(e.currentTarget.currentTime)}
						onDurationChange={(e) => setDuration(e.currentTarget.duration)}
						onLoadedMetadata={(e) => setDuration(e.currentTarget.duration)}
					/>
				</div>
			</div>

			{/* Subtitles */}
			{subtitlesEnabled && (
				<Subtitles file={file} videoElement={videoRef.current} settings={subtitleSettings} />
			)}

			{/* Subtitle Settings Menu */}
			<SubtitleSettingsMenu
				isOpen={showSubtitleSettings}
				settings={subtitleSettings}
				onSettingsChange={setSubtitleSettings}
				onClose={() => setShowSubtitleSettings(false)}
			/>

			{/* Controls Overlay */}
			<AnimatePresence>
				{showControls && (
					<motion.div
						initial={{ opacity: 0 }}
						animate={{ opacity: 1 }}
						exit={{ opacity: 0 }}
						transition={{ duration: 0.2 }}
						className="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/80 via-black/40 to-transparent px-4 pb-4 pt-16"
					>
						{/* Timeline Scrubber Preview */}
						{timelineHover && (
							<TimelineScrubber
								file={file}
								hoverPercent={timelineHover.percent}
								mouseX={timelineHover.mouseX}
								duration={duration}
							/>
						)}

						{/* Progress Bar with Thick Hover Area */}
						<div
							className="group mb-3 cursor-pointer relative py-2 -my-2"
							onMouseDown={(e) => {
								setSeeking(true);
								handleSeek(e);
							}}
							onMouseMove={(e) => {
								if (seeking) {
									handleSeek(e);
								} else {
									handleTimelineHover(e);
								}
							}}
							onMouseEnter={handleTimelineHover}
							onMouseUp={() => setSeeking(false)}
							onMouseLeave={() => {
								setSeeking(false);
								setTimelineHover(null);
							}}
						>
							<div className="relative h-1 w-full overflow-hidden rounded-full bg-white/20 transition-all group-hover:h-1.5">
								{/* Progress */}
								<div
									className="absolute left-0 top-0 h-full bg-accent transition-all"
									style={{ width: `${(currentTime / duration) * 100}%` }}
								/>

								{/* Scrubber */}
								<div
									className="absolute top-1/2 -translate-y-1/2 transition-all"
									style={{ left: `${(currentTime / duration) * 100}%` }}
								>
									<div className="size-3 -translate-x-1/2 rounded-full bg-accent opacity-0 shadow-lg transition-opacity group-hover:opacity-100" />
								</div>
							</div>
						</div>

						{/* Controls Bar */}
						<div className="flex items-center gap-3">
							{/* Play/Pause */}
							<button
								onClick={togglePlay}
								className="rounded-md p-2 text-white transition-colors hover:bg-white/10"
							>
								{playing ? <Pause size={20} weight="fill" /> : <Play size={20} weight="fill" />}
							</button>

							{/* Time */}
							<div className="text-sm font-medium text-white">
								{formatTime(currentTime)} / {formatTime(duration)}
							</div>

							<div className="flex-1" />

							{/* Subtitles Controls */}
							{hasSubs && (
								<div className="flex items-center gap-1">
									<button
										onClick={() => setSubtitlesEnabled(!subtitlesEnabled)}
										className={`rounded-md p-2 transition-colors ${
											subtitlesEnabled
												? 'bg-accent/20 text-accent'
												: 'text-white hover:bg-white/10'
										}`}
										title="Toggle subtitles"
									>
										<ClosedCaptioning size={20} weight="fill" />
									</button>
									{subtitlesEnabled && (
										<button
											onClick={() => setShowSubtitleSettings(!showSubtitleSettings)}
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
									onClick={zoomOut}
									disabled={zoom <= 1}
									className="rounded-md p-2 text-white transition-colors hover:bg-white/10 disabled:opacity-30"
									title="Zoom out (-)"
								>
									<MagnifyingGlassMinus size={18} weight="bold" />
								</button>
								<button
									onClick={zoomIn}
									disabled={zoom >= 5}
									className="rounded-md p-2 text-white transition-colors hover:bg-white/10 disabled:opacity-30"
									title="Zoom in (+)"
								>
									<MagnifyingGlassPlus size={18} weight="bold" />
								</button>
								{zoom > 1 && (
									<button
										onClick={reset}
										className="rounded-md p-2 text-white transition-colors hover:bg-white/10"
										title="Reset zoom (0)"
									>
										<ArrowCounterClockwise size={18} weight="bold" />
									</button>
								)}
							</div>

							{/* Volume */}
							<div className="group flex items-center gap-2">
								<button
									onClick={() => setMuted(!muted)}
									className="rounded-md p-2 text-white transition-colors hover:bg-white/10"
								>
									{muted || volume === 0 ? (
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
										value={volume}
										onChange={(e) => setVolume(parseFloat(e.target.value))}
										className="h-1 w-full cursor-pointer appearance-none rounded-full bg-white/20 [&::-webkit-slider-thumb]:size-3 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-white"
									/>
								</div>
							</div>

							{/* Fullscreen */}
							<button
								onClick={toggleFullscreen}
								className="rounded-md p-2 text-white transition-colors hover:bg-white/10"
							>
								<ArrowsOut size={20} weight="bold" />
							</button>
						</div>
					</motion.div>
				)}
			</AnimatePresence>
		</div>
	);
}
