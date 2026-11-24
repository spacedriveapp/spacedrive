import { useState, useRef, useEffect } from "react";
import {
	Play,
	Pause,
	SpeakerHigh,
	SpeakerSlash,
	SkipBack,
	SkipForward,
} from "@phosphor-icons/react";
import { motion } from "framer-motion";
import type { File } from "@sd/ts-client";
import { activeSegmentIndex } from "./TranscriptPanel";
import { useMediaSidecars } from "./useMediaSidecars";
import { useStartSeconds } from "./useStartSeconds";

interface AudioPlayerProps {
	src: string;
	file: File;
	startSeconds?: number;
}

function formatTime(seconds: number): string {
	const mins = Math.floor(seconds / 60);
	const secs = Math.floor(seconds % 60);
	return `${mins}:${secs.toString().padStart(2, "0")}`;
}

export function AudioPlayer({ src, file, startSeconds }: AudioPlayerProps) {
	const audioRef = useRef<HTMLAudioElement>(null);
	useStartSeconds(audioRef, file.id, startSeconds);
	const lyricsContainerRef = useRef<HTMLDivElement>(null);
	const [playing, setPlaying] = useState(false);
	const [currentTime, setCurrentTime] = useState(0);
	const [duration, setDuration] = useState(0);
	const [volume, setVolume] = useState(1);
	const [muted, setMuted] = useState(false);
	const [seeking, setSeeking] = useState(false);
	const sidecars = useMediaSidecars(file);
	const segments = sidecars?.transcript?.segments ?? [];
	const currentCueIndex = activeSegmentIndex(segments, currentTime * 1000);

	// Keep the line being spoken in view.
	useEffect(() => {
		if (currentCueIndex < 0) return;
		const line = lyricsContainerRef.current?.children[currentCueIndex] as
			| HTMLElement
			| undefined;
		line?.scrollIntoView({ behavior: "smooth", block: "center" });
	}, [currentCueIndex]);

	useEffect(() => {
		if (!audioRef.current) return;
		audioRef.current.volume = volume;
	}, [volume]);

	useEffect(() => {
		if (!audioRef.current) return;
		audioRef.current.muted = muted;
	}, [muted]);

	// Keyboard shortcuts
	useEffect(() => {
		const handleKeyDown = (e: KeyboardEvent) => {
			if (!audioRef.current) return;

			switch (e.code) {
				case "Space":
					e.preventDefault();
					togglePlay();
					break;
				case "ArrowLeft":
					e.preventDefault();
					audioRef.current.currentTime = Math.max(
						0,
						audioRef.current.currentTime - 10,
					);
					break;
				case "ArrowRight":
					e.preventDefault();
					audioRef.current.currentTime = Math.min(
						duration,
						audioRef.current.currentTime + 10,
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
					setMuted((m) => !m);
					break;
			}
		};

		window.addEventListener("keydown", handleKeyDown);
		return () => window.removeEventListener("keydown", handleKeyDown);
	}, [duration, playing]);

	const togglePlay = () => {
		if (!audioRef.current) return;
		if (playing) {
			audioRef.current.pause();
		} else {
			audioRef.current.play();
		}
	};

	const handleSeek = (e: React.MouseEvent<HTMLDivElement>) => {
		if (!audioRef.current) return;
		const rect = e.currentTarget.getBoundingClientRect();
		const percent = (e.clientX - rect.left) / rect.width;
		audioRef.current.currentTime = percent * duration;
	};

	const skipBack = () => {
		if (!audioRef.current) return;
		audioRef.current.currentTime = Math.max(
			0,
			audioRef.current.currentTime - 10,
		);
	};

	const skipForward = () => {
		if (!audioRef.current) return;
		audioRef.current.currentTime = Math.min(
			duration,
			audioRef.current.currentTime + 10,
		);
	};

	return (
		<div className="relative flex h-full w-full flex-col">
			{/* Hidden audio element */}
			<audio
				ref={audioRef}
				src={src}
				autoPlay
				onPlay={() => setPlaying(true)}
				onPause={() => setPlaying(false)}
				onTimeUpdate={(e) =>
					!seeking && setCurrentTime(e.currentTarget.currentTime)
				}
				onDurationChange={(e) => setDuration(e.currentTarget.duration)}
				onLoadedMetadata={(e) => setDuration(e.currentTarget.duration)}
			/>

			{/* Main content area - Lyrics only */}
			<div className="flex flex-1 overflow-hidden">
				<div className="relative flex min-w-0 flex-1 items-center justify-center p-8">
					<div className="absolute inset-0 flex w-full items-center justify-center p-8">
						{segments.length > 0 ? (
							<div
								ref={lyricsContainerRef}
								className="max-h-full w-full space-y-6 overflow-y-auto px-4 scrollbar-hide"
								style={{
									scrollbarWidth: "none",
									msOverflowStyle: "none",
								}}
							>
								{segments.map((segment, index) => {
									const isActive = index === currentCueIndex;
									return (
										<div
											key={segment.id}
											className="flex justify-center"
											onClick={() => {
												if (audioRef.current) {
													audioRef.current.currentTime =
														segment.start_ms / 1000;
												}
											}}
										>
											<motion.div
												initial={{ opacity: 0, y: 20 }}
												animate={{ opacity: 1, y: 0 }}
												transition={{
													delay: Math.min(index, 20) * 0.05,
												}}
												className={`cursor-pointer text-center text-2xl transition-all duration-300 ${
													isActive
														? "font-bold text-white"
														: "text-white/40 hover:text-white/60"
												}`}
												style={{
													transform: isActive ? "scale(1.15)" : "scale(1)",
													transformOrigin: "center",
												}}
											>
												{segment.text}
											</motion.div>
										</div>
									);
								})}
							</div>
						) : (
							<div className="text-center">
								<div className="mb-4 text-6xl font-bold text-white/30">♪</div>
								<p className="text-white/50">No transcript</p>
							</div>
						)}
					</div>
				</div>
			</div>

			{/* Bottom: Audio Controls */}
			<div className="px-6 py-4">
				{/* Progress Bar */}
				<div
					className="group mb-4 cursor-pointer"
					onMouseDown={(e) => {
						setSeeking(true);
						handleSeek(e);
					}}
					onMouseMove={(e) => seeking && handleSeek(e)}
					onMouseUp={() => setSeeking(false)}
					onMouseLeave={() => setSeeking(false)}
				>
					<div className="relative h-1.5 w-full overflow-hidden rounded-full bg-white/20 transition-all group-hover:h-2">
						{/* Progress */}
						<div
							className="bg-accent absolute left-0 top-0 h-full transition-all"
							style={{
								width: `${(currentTime / duration) * 100}%`,
							}}
						/>

						{/* Scrubber */}
						<div
							className="absolute top-1/2 -translate-y-1/2 transition-all"
							style={{
								left: `${(currentTime / duration) * 100}%`,
							}}
						>
							<div className="bg-accent size-3.5 -translate-x-1/2 rounded-full opacity-0 shadow-lg transition-opacity group-hover:opacity-100" />
						</div>
					</div>
				</div>

				{/* Controls */}
				<div className="flex items-center">
					{/* Left side - Fixed width */}
					<div className="flex w-[200px] items-center gap-3">
						{/* Time */}
						<div className="text-sm font-medium tabular-nums text-white/70">
							{formatTime(currentTime)}
						</div>
					</div>

					{/* Center - Playback Controls */}
					<div className="flex flex-1 items-center justify-center gap-2">
						<button
							onClick={skipBack}
							className="rounded-full p-2 text-white/70 transition-colors hover:bg-white/10 hover:text-white"
							title="Skip back 10s"
						>
							<SkipBack size={24} weight="fill" />
						</button>

						<button
							onClick={togglePlay}
							className="bg-accent hover:bg-accent/90 rounded-full p-3 text-white shadow-lg transition-all hover:scale-105"
						>
							{playing ? (
								<Pause size={28} weight="fill" />
							) : (
								<Play size={28} weight="fill" className="translate-x-0.5" />
							)}
						</button>

						<button
							onClick={skipForward}
							className="rounded-full p-2 text-white/70 transition-colors hover:bg-white/10 hover:text-white"
							title="Skip forward 10s"
						>
							<SkipForward size={24} weight="fill" />
						</button>
					</div>

					{/* Right side - Fixed width matching left */}
					<div className="flex w-[200px] items-center justify-end gap-3">
						{/* Time remaining */}
						<div className="text-sm font-medium tabular-nums text-white/70">
							-{formatTime(duration - currentTime)}
						</div>

						{/* Volume */}
						<div className="flex items-center gap-2">
							<button
								onClick={() => setMuted(!muted)}
								className="rounded-md p-2 text-white/70 transition-colors hover:bg-white/10 hover:text-white"
							>
								{muted || volume === 0 ? (
									<SpeakerSlash size={20} weight="fill" />
								) : (
									<SpeakerHigh size={20} weight="fill" />
								)}
							</button>

							{/* Volume Slider */}
							<div className="w-20">
								<input
									type="range"
									min="0"
									max="1"
									step="0.01"
									value={volume}
									onChange={(e) => setVolume(parseFloat(e.target.value))}
									className="[&::-webkit-slider-thumb]:bg-accent h-1 w-full cursor-pointer appearance-none rounded-full bg-white/20 [&::-webkit-slider-thumb]:size-3 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:shadow-lg [&::-webkit-slider-thumb]:transition-all [&::-webkit-slider-thumb]:hover:scale-110"
								/>
							</div>
						</div>
					</div>
				</div>
			</div>
		</div>
	);
}
