import { useState, useRef, useEffect, useCallback } from "react";
import type { File } from "@sd/ts-client";
import { Subtitles, type SubtitleSettings } from "./Subtitles";
import { SubtitleSettingsMenu } from "./SubtitleSettingsMenu";
import { useZoomPan } from "./useZoomPan";
import type {
	VideoControlsState,
	VideoControlsCallbacks,
} from "./VideoControls";

interface VideoPlayerProps {
	src: string;
	file: File;
	onZoomChange?: (isZoomed: boolean) => void;
	onControlsStateChange?: (state: VideoControlsState) => void;
	onShowControlsChange?: (show: boolean) => void;
	getCallbacks?: (callbacks: VideoControlsCallbacks) => void;
}

export function VideoPlayer({
	src,
	file,
	onZoomChange,
	onControlsStateChange,
	onShowControlsChange,
	getCallbacks,
}: VideoPlayerProps) {
	const videoRef = useRef<HTMLVideoElement>(null);
	const containerRef = useRef<HTMLDivElement>(null);
	const videoContainerRef = useRef<HTMLDivElement>(null);
	const [playing, setPlaying] = useState(false);
	const [currentTime, setCurrentTime] = useState(0);
	const [duration, setDuration] = useState(0);
	const [volume, setVolume] = useState(() => {
		const saved = localStorage.getItem("sd-video-volume");
		return saved ? parseFloat(saved) : 1;
	});
	const [muted, setMuted] = useState(() => {
		const saved = localStorage.getItem("sd-video-muted");
		return saved === "true";
	});
	const [loop, setLoop] = useState(false);
	const [showControls, setShowControls] = useState(true);
	const [seeking, setSeeking] = useState(false);
	const [subtitlesEnabled, setSubtitlesEnabled] = useState(true);
	const [showSubtitleSettings, setShowSubtitleSettings] = useState(false);
	const [subtitleSettings, setSubtitleSettings] = useState<SubtitleSettings>({
		fontSize: 1.5,
		position: "bottom",
		backgroundOpacity: 0.9,
	});
	const [timelineHover, setTimelineHover] = useState<{
		percent: number;
		mouseX: number;
	} | null>(null);
	const hideControlsTimeout = useRef<number | undefined>(undefined);
	const { zoom, zoomIn, zoomOut, reset, isZoomed, transform } =
		useZoomPan(videoContainerRef);

	// Expose controls state to parent
	useEffect(() => {
		onControlsStateChange?.({
			playing,
			currentTime,
			duration,
			volume,
			muted,
			loop,
			zoom,
			subtitlesEnabled,
			showSubtitleSettings,
			seeking,
			timelineHover,
		});
	}, [
		playing,
		currentTime,
		duration,
		volume,
		muted,
		loop,
		zoom,
		subtitlesEnabled,
		showSubtitleSettings,
		seeking,
		timelineHover,
		onControlsStateChange,
	]);

	// Expose showControls state to parent
	useEffect(() => {
		onShowControlsChange?.(showControls);
	}, [showControls, onShowControlsChange]);

	// Notify parent of zoom state changes
	useEffect(() => {
		onZoomChange?.(isZoomed);
	}, [isZoomed, onZoomChange]);

	const togglePlay = useCallback(() => {
		if (!videoRef.current) return;
		if (playing) {
			videoRef.current.pause();
		} else {
			videoRef.current.play();
		}
	}, [playing]);

	const handleSeek = useCallback(
		(e: React.MouseEvent<HTMLDivElement>) => {
			if (!videoRef.current) return;
			const rect = e.currentTarget.getBoundingClientRect();
			const percent = (e.clientX - rect.left) / rect.width;
			videoRef.current.currentTime = percent * duration;
		},
		[duration],
	);

	const handleTimelineHover = useCallback(
		(e: React.MouseEvent<HTMLDivElement>) => {
			const rect = e.currentTarget.getBoundingClientRect();
			const percent = (e.clientX - rect.left) / rect.width;
			setTimelineHover({ percent, mouseX: e.clientX });
		},
		[],
	);

	const toggleFullscreen = useCallback(() => {
		if (!containerRef.current) return;
		if (document.fullscreenElement) {
			document.exitFullscreen();
		} else {
			containerRef.current.requestFullscreen();
		}
	}, []);

	const handleTimelineLeave = useCallback(() => {
		setSeeking(false);
		setTimelineHover(null);
	}, []);

	const handleSeekingStart = useCallback(() => setSeeking(true), []);
	const handleSeekingEnd = useCallback(() => setSeeking(false), []);
	const handleMuteToggle = useCallback(() => setMuted((m) => !m), []);
	const handleLoopToggle = useCallback(() => setLoop((l) => !l), []);
	const handleSubtitlesToggle = useCallback(
		() => setSubtitlesEnabled((s) => !s),
		[],
	);
	const handleSubtitleSettingsToggle = useCallback(
		() => setShowSubtitleSettings((s) => !s),
		[],
	);

	// Show controls on mouse move, hide after 1s of inactivity
	const handleMouseMove = useCallback(() => {
		setShowControls(true);
		if (hideControlsTimeout.current) {
			clearTimeout(hideControlsTimeout.current);
		}
		if (playing) {
			hideControlsTimeout.current = setTimeout(() => {
				setShowControls(false);
			}, 1000);
		}
	}, [playing]);

	// Provide callbacks to parent
	useEffect(() => {
		getCallbacks?.({
			onTogglePlay: togglePlay,
			onSeek: handleSeek,
			onTimelineHover: handleTimelineHover,
			onTimelineLeave: handleTimelineLeave,
			onSeekingStart: handleSeekingStart,
			onSeekingEnd: handleSeekingEnd,
			onVolumeChange: setVolume,
			onMuteToggle: handleMuteToggle,
			onLoopToggle: handleLoopToggle,
			onZoomIn: zoomIn,
			onZoomOut: zoomOut,
			onZoomReset: reset,
			onSubtitlesToggle: handleSubtitlesToggle,
			onSubtitleSettingsToggle: handleSubtitleSettingsToggle,
			onFullscreenToggle: toggleFullscreen,
			onMouseMove: handleMouseMove,
		});
	}, [
		togglePlay,
		handleSeek,
		handleTimelineHover,
		handleTimelineLeave,
		handleSeekingStart,
		handleSeekingEnd,
		handleMuteToggle,
		handleLoopToggle,
		handleSubtitlesToggle,
		handleSubtitleSettingsToggle,
		toggleFullscreen,
		handleMouseMove,
		zoomIn,
		zoomOut,
		reset,
		getCallbacks,
	]);

	// Keyboard shortcuts
	useEffect(() => {
		const handleKeyDown = (e: KeyboardEvent) => {
			if (!videoRef.current) return;

			switch (e.code) {
				case "Space":
					e.preventDefault();
					togglePlay();
					break;
				case "ArrowLeft":
					e.preventDefault();
					videoRef.current.currentTime = Math.max(
						0,
						videoRef.current.currentTime - 5,
					);
					break;
				case "ArrowRight":
					e.preventDefault();
					videoRef.current.currentTime = Math.min(
						duration,
						videoRef.current.currentTime + 5,
					);
					break;
				case "ArrowUp":
					e.preventDefault();
					setVolume((v) => Math.min(1, v + 0.1));
					break;
				case "ArrowDown":
					e.preventDefault();
					setVolume((v) => Math.max(0, v - 0.1));
					break;
				case "KeyM":
					e.preventDefault();
					handleMuteToggle();
					break;
				case "KeyF":
					e.preventDefault();
					toggleFullscreen();
					break;
				case "KeyC":
					e.preventDefault();
					handleSubtitlesToggle();
					break;
				case "KeyL":
					e.preventDefault();
					handleLoopToggle();
					break;
			}
		};

		window.addEventListener("keydown", handleKeyDown);
		return () => window.removeEventListener("keydown", handleKeyDown);
	}, [
		duration,
		togglePlay,
		toggleFullscreen,
		handleMuteToggle,
		handleSubtitlesToggle,
		handleLoopToggle,
	]);

	// Sync video element state and persist to localStorage
	useEffect(() => {
		if (!videoRef.current) return;
		videoRef.current.volume = volume;
		localStorage.setItem("sd-video-volume", volume.toString());
	}, [volume]);

	useEffect(() => {
		if (!videoRef.current) return;
		videoRef.current.muted = muted;
		localStorage.setItem("sd-video-muted", muted.toString());
	}, [muted]);

	useEffect(() => {
		if (!videoRef.current) return;
		videoRef.current.loop = loop;
	}, [loop]);

	return (
		<div
			ref={containerRef}
			className="relative flex h-full w-full items-center justify-center bg-black"
			onMouseMove={handleMouseMove}
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
				className={`relative flex h-full w-full items-center justify-center ${isZoomed ? "overflow-visible" : "overflow-hidden"}`}
			>
				<div
					style={transform}
					className="flex items-center justify-center"
				>
					<video
						ref={videoRef}
						src={src}
						autoPlay
						playsInline
						className="max-h-screen max-w-screen"
						onPlay={() => setPlaying(true)}
						onPause={() => setPlaying(false)}
						onTimeUpdate={(e) =>
							!seeking &&
							setCurrentTime(e.currentTarget.currentTime)
						}
						onDurationChange={(e) =>
							setDuration(e.currentTarget.duration)
						}
						onLoadedMetadata={(e) =>
							setDuration(e.currentTarget.duration)
						}
					/>
				</div>
			</div>

			{/* Subtitles */}
			{subtitlesEnabled && (
				<div className="absolute inset-0 z-10 pointer-events-none">
					<Subtitles
						file={file}
						videoElement={videoRef.current}
						settings={subtitleSettings}
					/>
				</div>
			)}

			{/* Subtitle Settings Menu */}
			<SubtitleSettingsMenu
				isOpen={showSubtitleSettings}
				settings={subtitleSettings}
				onSettingsChange={setSubtitleSettings}
				onClose={() => setShowSubtitleSettings(false)}
			/>
		</div>
	);
}
