import {
	CaretDown,
	Microphone,
	SpeakerHigh,
	Stop,
} from "@phosphor-icons/react";
import { BallBlue } from "@sd/assets/images";
import { Popover, usePopover } from "@sd/ui";
import { useEffect, useMemo, useRef, useState } from "react";
import Orb from "../components/Orb";
import { usePlatform } from "../contexts/PlatformContext";

type VoiceState = "idle" | "recording" | "processing" | "speaking";

const OVERLAY_WINDOW_LABEL = "voice-overlay";
const OVERLAY_WIDTH = 520;

const voiceProfiles = ["Alloy", "Verse", "Orbit"];

export function VoiceOverlay() {
	const platform = usePlatform();
	const profileSelector = usePopover();
	const containerRef = useRef<HTMLDivElement>(null);
	const [expanded, setExpanded] = useState(false);
	const [voiceState, setVoiceState] = useState<VoiceState>("idle");
	const [statusText, setStatusText] = useState("Press Option+Shift+Space to talk");
	const [profile, setProfile] = useState("Alloy");
	const [transcript, setTranscript] = useState<Array<{ role: string; text: string }>>([]);
	const [phase, setPhase] = useState(0);

	useEffect(() => {
		document.documentElement.classList.add("overlay-window");
		document.body.classList.add("overlay-window");
		document.getElementById("root")?.classList.add("overlay-window");

		return () => {
			document.documentElement.classList.remove("overlay-window");
			document.body.classList.remove("overlay-window");
			document.getElementById("root")?.classList.remove("overlay-window");
		};
	}, []);

	useEffect(() => {
		if (!platform.resizeWindow) return;
		const element = containerRef.current;
		if (!element) return;

		const observer = new ResizeObserver((entries) => {
			const height =
				entries[0]?.borderBoxSize?.[0]?.blockSize ?? entries[0]?.contentRect.height ?? 0;
			if (height > 0) {
				void platform.resizeWindow?.(OVERLAY_WINDOW_LABEL, OVERLAY_WIDTH, Math.ceil(height));
			}
		});

		observer.observe(element);
		return () => observer.disconnect();
	}, [platform]);

	useEffect(() => {
		if (voiceState === "idle") return;

		const interval = window.setInterval(() => {
			setPhase((value) => value + 0.28);
		}, 60);

		return () => window.clearInterval(interval);
	}, [voiceState]);

	useEffect(() => {
		const handleKeyDown = (event: KeyboardEvent) => {
			if (event.code === "Space" && event.altKey && event.shiftKey && voiceState === "idle") {
				event.preventDefault();
				void handleStartRecording();
			}
		};

		const handleKeyUp = (event: KeyboardEvent) => {
			if (event.code === "Space" && event.altKey && event.shiftKey && voiceState === "recording") {
				event.preventDefault();
				void handleStopRecording();
			}
		};

		window.addEventListener("keydown", handleKeyDown);
		window.addEventListener("keyup", handleKeyUp);

		return () => {
			window.removeEventListener("keydown", handleKeyDown);
			window.removeEventListener("keyup", handleKeyUp);
		};
	}, [voiceState]);

	async function handleStartRecording() {
		if (voiceState !== "idle") return;
		setVoiceState("recording");
		setStatusText("Listening...");
	}

	async function handleStopRecording() {
		if (voiceState !== "recording") return;
		setVoiceState("processing");
		setStatusText("Processing...");
		setTranscript((current) => [...current, { role: "user", text: "[voice message]" }]);

		window.setTimeout(() => {
			setVoiceState("speaking");
			setStatusText("I can help plan work, review projects, and coordinate tasks across your devices.");
			setTranscript((current) => [
				...current,
				{
					role: "assistant",
					text: "I can help plan work, review projects, and coordinate tasks across your devices.",
				},
			]);

			window.setTimeout(() => {
				setVoiceState("idle");
				setStatusText("Press Option+Shift+Space to talk");
			}, 2200);
		}, 900);
	}

	function handlePrimaryAction(event: React.MouseEvent<HTMLButtonElement>) {
		event.stopPropagation();
		if (voiceState === "idle") {
			void handleStartRecording();
		} else if (voiceState === "speaking") {
			setVoiceState("idle");
			setStatusText("Press Option+Shift+Space to talk");
		}
	}

	const activeEnergy =
		voiceState === "recording" ? 0.82 + Math.sin(phase * 1.4) * 0.08 : voiceState === "speaking" ? 0.58 + Math.sin(phase) * 0.1 : 0;

	const waveColor =
		voiceState === "recording" ? "#70b8ff" : voiceState === "speaking" ? "#ba5cf6" : "#6b7280";

	const activeSpectrumLevels = useMemo(() => {
		return Array.from({ length: 45 }, (_, index) => {
			if (voiceState === "idle") return 0;
			const wave = Math.sin(phase + index * 0.35) * 0.5 + 0.5;
			const secondary = Math.sin(phase * 0.5 + index * 0.18) * 0.5 + 0.5;
			return Math.min(1, wave * 0.7 + secondary * 0.3);
		});
	}, [phase, voiceState]);

	const haloStyle =
		voiceState === "recording"
			? {
				background: `radial-gradient(circle, rgba(88,166,255,${0.2 + activeEnergy * 0.22}) 0%, rgba(88,166,255,${0.08 + activeEnergy * 0.12}) 34%, transparent 72%)`,
				transform: `scale(${1 + activeEnergy * 0.16})`,
			}
			: voiceState === "speaking"
				? {
					background: `radial-gradient(circle, rgba(186,92,246,${0.18 + activeEnergy * 0.24}) 0%, rgba(186,92,246,${0.06 + activeEnergy * 0.14}) 34%, transparent 72%)`,
					transform: `scale(${1 + activeEnergy * 0.18})`,
				}
				: {
					background: "radial-gradient(circle, rgba(255,255,255,0.08) 0%, rgba(255,255,255,0.03) 34%, transparent 72%)",
					transform: "scale(1)",
				};

	return (
		<div ref={containerRef} className="flex w-screen flex-col items-center justify-end select-none" style={{ background: "transparent" }}>
			{expanded && (
				<div className="mb-2 w-full max-w-[500px] overflow-hidden rounded-2xl border border-white/10 bg-app/95 shadow-2xl backdrop-blur-xl">
					<div className="flex items-start justify-between gap-3 border-b border-white/5 px-4 py-2.5">
						<div className="flex items-center gap-2">
							<div className="h-3 w-3 rounded-full bg-accent" />
							<span className="text-xs font-medium text-ink">Spacebot</span>
						</div>
						<div className="flex min-w-0 items-center gap-2">
							<div className="w-[160px]">
								<Popover
									popover={profileSelector}
									trigger={
										<button className="flex h-8 w-full items-center gap-2 rounded-full border border-white/10 bg-white/5 px-3 text-left text-[11px] font-medium text-ink-dull transition-colors hover:bg-white/10 hover:text-ink">
											<span className="flex-1 truncate text-left">{profile}</span>
											<CaretDown className="size-3" weight="bold" />
										</button>
									}
									align="end"
									sideOffset={8}
									className="p-2"
								>
									<div className="space-y-1">
										{voiceProfiles.map((option) => (
											<button
												key={option}
												onClick={() => {
													setProfile(option);
													profileSelector.setOpen(false);
												}}
												className="w-full rounded-md px-3 py-2 text-left text-sm text-ink transition-colors hover:bg-app-selected"
											>
												{option}
											</button>
										))}
									</div>
								</Popover>
							</div>
							<button onClick={() => setExpanded(false)} className="text-[11px] text-ink-faint transition-colors hover:text-ink">
								Collapse
							</button>
						</div>
					</div>

					<div className="max-h-[300px] overflow-y-auto p-4">
						{transcript.length === 0 ? (
							<p className="text-center text-xs text-ink-faint">Your conversation will appear here.</p>
						) : (
							<div className="flex flex-col gap-3">
								{transcript.map((entry, index) => (
									<div key={`${entry.role}-${index}`} className="flex flex-col gap-0.5">
										<span className="text-[11px] font-medium text-ink-faint">{entry.role === "user" ? "You" : "Spacebot"}</span>
										<p className="whitespace-pre-wrap text-xs leading-relaxed text-ink">{entry.text}</p>
									</div>
								))}
							</div>
						)}
					</div>
				</div>
			)}

			<div
				className={`voice-overlay-pill relative mb-2 flex w-full max-w-[460px] cursor-pointer items-center gap-2.5 overflow-hidden rounded-[20px] border px-3 py-2 shadow-2xl backdrop-blur-xl transition-all ${
					voiceState === "recording"
						? "border-sky-300/35 bg-sky-400/10"
						: voiceState === "speaking"
							? "border-violet-300/35 bg-violet-400/10"
							: "border-white/10 bg-app/95"
				}`}
				data-tauri-drag-region
				onClick={() => {
					if (voiceState === "idle") setExpanded((value) => !value);
				}}
			>
				<div className="pointer-events-none absolute inset-x-5 -bottom-5 -top-5 rounded-full blur-2xl transition-all duration-200" style={haloStyle} />

				<div className="relative flex h-9 w-9 flex-shrink-0 items-center justify-center">
					<div
						className={`absolute inset-0 transition-all duration-150 ${
							voiceState === "recording"
								? "bg-sky-400/12"
								: voiceState === "speaking"
									? "bg-violet-400/12"
									: "bg-transparent"
						}`}
						style={{ transform: `scale(${1 + activeEnergy * 0.22})` }}
					/>
					<div className="relative z-10 flex h-7 w-7 items-center justify-center text-ink">
						<BullLogoOrb />
					</div>
				</div>

				<div className="relative z-10 flex min-w-0 flex-1 flex-col gap-1">
					<div className="flex items-center">
						<span
							className={`rounded-full py-0.5 text-[8px] font-semibold uppercase tracking-[0.14em] ${
								voiceState === "recording"
									? "bg-sky-400/14 text-sky-200"
									: voiceState === "speaking"
										? "bg-violet-400/14 text-violet-200"
										: voiceState === "processing"
											? "bg-violet-400/14 text-violet-200"
											: "bg-white/6 text-ink-faint"
							}`}
						>
							{voiceState === "recording"
								? "Input live"
								: voiceState === "speaking"
									? "Reply live"
									: voiceState === "processing"
										? "Thinking"
										: "Voice ready"}
						</span>
					</div>
					<p className={`min-w-0 truncate text-[12px] leading-tight ${voiceState === "idle" ? "text-ink-faint" : "text-ink"}`}>
						{statusText}
					</p>
					<div className="relative z-10 h-9 overflow-hidden px-1">
						<SiriWaveform levels={activeSpectrumLevels} energy={activeEnergy} color={waveColor} active={voiceState === "recording" || voiceState === "speaking"} />
					</div>
				</div>

				<button
					onClick={handlePrimaryAction}
					className={`relative z-10 flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-full border transition-colors ${
						voiceState === "idle"
							? "border-white/10 bg-white/5 text-ink-faint hover:bg-white/10 hover:text-ink"
							: voiceState === "processing"
								? "animate-pulse border-violet-300/30 bg-violet-400/15 text-violet-100"
								: voiceState === "speaking"
									? "border-violet-300/30 bg-violet-400/15 text-violet-100"
									: "border-sky-300/30 bg-sky-400/15 text-sky-100"
					}`}
				>
					{voiceState === "speaking" ? <SpeakerHigh className="size-4" weight="fill" /> : <Microphone className="size-4" weight="fill" />}
				</button>

				{voiceState === "recording" && (
					<button
						onClick={(event) => {
							event.stopPropagation();
							void handleStopRecording();
						}}
						className="relative z-10 flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-full border border-sky-300/30 bg-sky-400/15 text-sky-100 transition-colors hover:bg-sky-400/20"
					>
						<Stop className="size-4" weight="fill" />
					</button>
				)}
			</div>
		</div>
	);
}

function BullLogoOrb() {
	return (
		<>
			<div className="absolute inset-[calc(5%-2px)] z-0" aria-hidden="true">
				<img src={BallBlue} alt="" className="h-full w-full object-contain" draggable={false} />
			</div>
			<div className="absolute inset-0 z-10" aria-hidden="true">
				<Orb palette="blue" hue={0} hoverIntensity={0} rotateOnHover={false} forceHoverState />
			</div>
		</>
	);
}

function SiriWaveform({
	levels,
	energy,
	color,
	active,
}: {
	levels: number[];
	energy: number;
	color: string;
	active: boolean;
}) {
	const width = 280;
	const height = 36;
	const centerY = height / 2;

	const smoothedLevels = useMemo(() => {
		if (levels.length === 0) {
			return Array.from({ length: 24 }, () => 0);
		}

		const bucketCount = 24;
		return Array.from({ length: bucketCount }, (_, bucketIndex) => {
			const start = Math.floor((bucketIndex / bucketCount) * levels.length);
			const end = Math.max(start + 1, Math.floor(((bucketIndex + 1) / bucketCount) * levels.length));
			const slice = levels.slice(start, end);
			const average = slice.reduce((sum, value) => sum + value, 0) / slice.length;
			return Math.min(1, average);
		});
	}, [levels]);

	const path = useMemo(() => {
		const sampleCount = 88;
		const points = Array.from({ length: sampleCount + 1 }, (_, index) => {
			const progress = index / sampleCount;
			const x = progress * width;
			const levelIndex = Math.min(smoothedLevels.length - 1, Math.floor(progress * smoothedLevels.length));
			const fft = smoothedLevels[levelIndex] ?? 0;
			const envelope = Math.pow(Math.sin(progress * Math.PI), 1.35);
			const amplitude = active ? 3.2 + energy * 9.5 + fft * 12.5 : 1.35;
			const primary = Math.sin(progress * Math.PI * 3.15);
			const secondary = Math.sin(progress * Math.PI * 1.7) * 0.4;
			const y = centerY - (primary + secondary) * amplitude * envelope;
			return { x, y };
		});

		return points.reduce((current, point, index, all) => {
			if (index === 0) return `M ${point.x.toFixed(2)} ${point.y.toFixed(2)}`;
			const previous = all[index - 1];
			const controlX = ((previous.x + point.x) / 2).toFixed(2);
			const controlY = ((previous.y + point.y) / 2).toFixed(2);
			return `${current} Q ${previous.x.toFixed(2)} ${previous.y.toFixed(2)}, ${controlX} ${controlY}`;
		}, "");
	}, [active, centerY, energy, smoothedLevels]);

	return (
		<svg viewBox={`0 0 ${width} ${height}`} className="h-full w-full" preserveAspectRatio="none" aria-hidden="true">
			<path d={`M 0 ${centerY} L ${width} ${centerY}`} stroke={color} strokeOpacity={active ? 0.14 : 0.08} strokeWidth="1" />
			<path d={path} fill="none" stroke={color} strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" opacity={active ? 0.92 : 0.24} />
		</svg>
	);
}
