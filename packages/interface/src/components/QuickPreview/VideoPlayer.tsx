import { useState, useRef, useEffect, useCallback } from "react";
import type { File } from "@sd/ts-client";
import { Subtitles, type SubtitleSettings } from "./Subtitles";
import { SubtitleSettingsMenu } from "./SubtitleSettingsMenu";
import { TranscriptPanel } from "./TranscriptPanel";
import { useMediaSidecars, useProxyUrl } from "./useMediaSidecars";
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
	const videoRef = useRef<HTMLVideoElement | null>(null);
	const [videoElement, setVideoElement] = useState<HTMLVideoElement | null>(
		null,
	);
	const attachVideo = useCallback((element: HTMLVideoElement | null) => {
		videoRef.current = element;
		setVideoElement(element);
	}, []);
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
	const [transcriptOpen, setTranscriptOpen] = useState(true);
	const sidecars = useMediaSidecars(file);
	const segments = sidecars?.transcript?.segments ?? [];
	const hasTranscript = segments.length > 0;
	const proxyUrl = useProxyUrl(sidecars);
	// The original stays the source until the webview shows it cannot decode
	// it; the proxy then takes over at the same position. The renderer keys
	// this component by source, so a new file starts with this state fresh.
	const [originalFailed, setOriginalFailed] = useState(false);
	const resumeAt = useRef<number | null>(null);
	const playingProxy = originalFailed && proxyUrl !== null;
	const videoSrc = playingProxy ? proxyUrl : src;
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
	const { zoom, zoomIn, zoomOut, reset, isZoomed, transform } = useZoomPan(
		videoContainerRef as React.RefObject<HTMLElement>,
	);

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
			transcriptOpen: hasTranscript && transcriptOpen,
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
		hasTranscript,
		transcriptOpen,
		seeking,
		timelineHover,
		onControlsStateChange,
	]);

	const handleOriginalUnplayable = useCallback(
		(video: HTMLVideoElement) => {
			if (originalFailed) return;
			resumeAt.current = video.currentTime;
			setOriginalFailed(true);
		},
		[originalFailed],
	);

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
	const handleTranscriptToggle = useCallback(
		() => setTranscriptOpen((open) => !open),
		[],
	);

	// Show controls on mouse move, hide after 1s of inactivity
	const handleMouseMove = useCallback(() => {
		setShowControls(true);
		if (hideControlsTimeout.current) {
			clearTimeout(hideControlsTimeout.current);
		}
		if (playing) {
			hideControlsTimeout.current = window.setTimeout(() => {
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
			onTranscriptToggle: handleTranscriptToggle,
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
		handleTranscriptToggle,
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
				case "KeyT":
					e.preventDefault();
					handleTranscriptToggle();
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
		handleTranscriptToggle,
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
			className="relative flex h-full w-full bg-black"
			onMouseMove={handleMouseMove}
		>
			<div className="relative flex h-full min-w-0 flex-1 items-center justify-center">
				{/* Zoom level indicator */}
				{zoom > 1 && (
					<div className="absolute left-4 top-4 z-20 rounded-lg bg-black/80 px-3 py-1.5 text-sm font-medium text-white backdrop-blur-xl">
						{Math.round(zoom * 100)}%
					</div>
				)}

				{playingProxy && zoom <= 1 && (
					<div
						className="absolute left-4 top-4 z-20 rounded-md bg-black/70 px-2 py-1 text-xs font-medium text-white/80 backdrop-blur-xl"
						title="This webview cannot decode the original, so its proxy is playing"
					>
						Proxy
					</div>
				)}

				{originalFailed && !proxyUrl && (
					<div className="absolute inset-x-0 top-1/2 z-20 -translate-y-1/2 px-8 text-center text-sm text-white/70">
						This video cannot be played here. Make a proxy to preview it.
					</div>
				)}

				{/* Video container with zoom/pan */}
				<div
					ref={videoContainerRef}
					className={`relative flex h-full w-full items-center justify-center ${isZoomed ? "overflow-visible" : "overflow-hidden"}`}
				>
					<div
						style={transform}
						className="flex max-w-full items-center justify-center"
					>
						<video
							ref={attachVideo}
							src={videoSrc}
							autoPlay
							playsInline
							className="max-h-screen max-w-full"
							onPlay={() => setPlaying(true)}
							onPause={() => setPlaying(false)}
							onTimeUpdate={(e) =>
								!seeking && setCurrentTime(e.currentTarget.currentTime)
							}
							onDurationChange={(e) => setDuration(e.currentTarget.duration)}
							onLoadedMetadata={(e) => {
								const video = e.currentTarget;
								setDuration(video.duration);
								// A codec the webview cannot decode can still
								// load, with sound and no picture.
								if (!playingProxy && video.videoWidth === 0) {
									handleOriginalUnplayable(video);
									return;
								}
								if (resumeAt.current !== null) {
									video.currentTime = resumeAt.current;
									resumeAt.current = null;
								}
							}}
							onError={(e) => {
								if (!playingProxy) {
									handleOriginalUnplayable(e.currentTarget);
								}
							}}
						/>
					</div>
				</div>

				{/* Subtitles */}
				{subtitlesEnabled && hasTranscript && (
					<div className="absolute inset-0 z-10 pointer-events-none">
						<Subtitles
							segments={segments}
							videoElement={videoElement}
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

			{hasTranscript && transcriptOpen && (
				<TranscriptPanel segments={segments} media={videoElement} />
			)}
		</div>
	);
}
