import {
  Pause,
  Play,
  SkipBack,
  SkipForward,
  SpeakerHigh,
  SpeakerSlash,
} from "@phosphor-icons/react";
import type { File } from "@sd/ts-client";
import { motion } from "framer-motion";
import { useEffect, useRef, useState } from "react";
import { useServer } from "../../contexts/ServerContext";

interface SubtitleCue {
  index: number;
  startTime: number;
  endTime: number;
  text: string;
}

interface AudioPlayerProps {
  src: string;
  file: File;
}

function formatTime(seconds: number): string {
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  return `${mins}:${secs.toString().padStart(2, "0")}`;
}

function parseSRT(srtContent: string): SubtitleCue[] {
  const cues: SubtitleCue[] = [];
  const blocks = srtContent.trim().split(/\n\s*\n/);

  for (const block of blocks) {
    const lines = block.trim().split("\n");
    if (lines.length < 3) continue;

    const index = Number.parseInt(lines[0], 10);
    const timecodeMatch = lines[1].match(
      /(\d{2}):(\d{2}):(\d{2}),(\d{3})\s*-->\s*(\d{2}):(\d{2}):(\d{2}),(\d{3})/
    );

    if (!timecodeMatch) continue;

    const startTime =
      Number.parseInt(timecodeMatch[1]) * 3600 +
      Number.parseInt(timecodeMatch[2]) * 60 +
      Number.parseInt(timecodeMatch[3]) +
      Number.parseInt(timecodeMatch[4]) / 1000;

    const endTime =
      Number.parseInt(timecodeMatch[5]) * 3600 +
      Number.parseInt(timecodeMatch[6]) * 60 +
      Number.parseInt(timecodeMatch[7]) +
      Number.parseInt(timecodeMatch[8]) / 1000;

    const text = lines.slice(2).join("\n");

    cues.push({ index, startTime, endTime, text });
  }

  return cues;
}

export function AudioPlayer({ src, file }: AudioPlayerProps) {
  const audioRef = useRef<HTMLAudioElement>(null);
  const lyricsContainerRef = useRef<HTMLDivElement>(null);
  const { buildSidecarUrl } = useServer();
  const [playing, setPlaying] = useState(false);
  const [currentTime, setCurrentTime] = useState(0);
  const [duration, setDuration] = useState(0);
  const [volume, setVolume] = useState(1);
  const [muted, setMuted] = useState(false);
  const [seeking, setSeeking] = useState(false);
  const [cues, setCues] = useState<SubtitleCue[]>([]);
  const [currentCueIndex, setCurrentCueIndex] = useState<number>(-1);

  // Load SRT transcripts if available
  useEffect(() => {
    const srtSidecar = file.sidecars?.find(
      (s) => s.kind === "transcript" && s.variant === "srt"
    );

    if (!(srtSidecar && file.content_identity?.uuid)) {
      return;
    }

    const extension = srtSidecar.format === "text" ? "txt" : srtSidecar.format;
    const srtUrl = buildSidecarUrl(
      file.content_identity.uuid,
      srtSidecar.kind,
      srtSidecar.variant,
      extension
    );

    if (!srtUrl) return;

    fetch(srtUrl)
      .then(async (res) => {
        if (!res.ok) return null;
        return res.text();
      })
      .then((srtContent) => {
        if (!srtContent) return;
        const parsed = parseSRT(srtContent);
        console.log("[AudioPlayer] Loaded", parsed.length, "lyric lines");
        setCues(parsed);
      })
      .catch((err) =>
        console.log("[AudioPlayer] Lyrics not available:", err.message)
      );
  }, [file, buildSidecarUrl]);

  // Sync lyrics with audio playback
  useEffect(() => {
    if (!audioRef.current || cues.length === 0) return;

    const updateLyrics = () => {
      const time = audioRef.current!.currentTime;
      const index = cues.findIndex(
        (cue) => time >= cue.startTime && time <= cue.endTime
      );

      if (index !== currentCueIndex) {
        setCurrentCueIndex(index);

        // Auto-scroll to active lyric
        if (index >= 0 && lyricsContainerRef.current) {
          const activeElement = lyricsContainerRef.current.children[
            index
          ] as HTMLElement;
          if (activeElement) {
            activeElement.scrollIntoView({
              behavior: "smooth",
              block: "center",
            });
          }
        }
      }
    };

    audioRef.current.addEventListener("timeupdate", updateLyrics);
    audioRef.current.addEventListener("seeked", updateLyrics);

    return () => {
      audioRef.current?.removeEventListener("timeupdate", updateLyrics);
      audioRef.current?.removeEventListener("seeked", updateLyrics);
    };
  }, [audioRef.current, cues, currentCueIndex]);

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
            audioRef.current.currentTime - 10
          );
          break;
        case "ArrowRight":
          e.preventDefault();
          audioRef.current.currentTime = Math.min(
            duration,
            audioRef.current.currentTime + 10
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
      audioRef.current.currentTime - 10
    );
  };

  const skipForward = () => {
    if (!audioRef.current) return;
    audioRef.current.currentTime = Math.min(
      duration,
      audioRef.current.currentTime + 10
    );
  };

  return (
    <div className="relative flex h-full w-full flex-col">
      {/* Hidden audio element */}
      <audio
        autoPlay
        onDurationChange={(e) => setDuration(e.currentTarget.duration)}
        onLoadedMetadata={(e) => setDuration(e.currentTarget.duration)}
        onPause={() => setPlaying(false)}
        onPlay={() => setPlaying(true)}
        onTimeUpdate={(e) =>
          !seeking && setCurrentTime(e.currentTarget.currentTime)
        }
        ref={audioRef}
        src={src}
      />

      {/* Main content area - Lyrics only */}
      <div className="flex flex-1 overflow-hidden">
        <div className="relative flex min-w-0 flex-1 items-center justify-center p-8">
          <div className="absolute inset-0 flex w-full items-center justify-center p-8">
            {cues.length > 0 ? (
              <div
                className="scrollbar-hide max-h-full w-full space-y-6 overflow-y-auto px-4"
                ref={lyricsContainerRef}
                style={{
                  scrollbarWidth: "none",
                  msOverflowStyle: "none",
                }}
              >
                {cues.map((cue, index) => {
                  const isActive = index === currentCueIndex;
                  return (
                    <div
                      className="flex justify-center"
                      key={cue.index}
                      onClick={() => {
                        if (audioRef.current) {
                          audioRef.current.currentTime = cue.startTime;
                        }
                      }}
                    >
                      <motion.div
                        animate={{ opacity: 1, y: 0 }}
                        className={`cursor-pointer text-center text-2xl transition-all duration-300 ${
                          isActive
                            ? "font-bold text-white"
                            : "text-white/40 hover:text-white/60"
                        }`}
                        initial={{ opacity: 0, y: 20 }}
                        style={{
                          transform: isActive ? "scale(1.15)" : "scale(1)",
                          transformOrigin: "center",
                        }}
                        transition={{
                          delay: index * 0.05,
                        }}
                      >
                        {cue.text}
                      </motion.div>
                    </div>
                  );
                })}
              </div>
            ) : (
              <div className="text-center">
                <div className="mb-4 font-bold text-6xl text-white/30">♪</div>
                <p className="text-white/50">No lyrics available</p>
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
          onMouseLeave={() => setSeeking(false)}
          onMouseMove={(e) => seeking && handleSeek(e)}
          onMouseUp={() => setSeeking(false)}
        >
          <div className="relative h-1.5 w-full overflow-hidden rounded-full bg-white/20 transition-all group-hover:h-2">
            {/* Progress */}
            <div
              className="absolute top-0 left-0 h-full bg-accent transition-all"
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
              <div className="size-3.5 -translate-x-1/2 rounded-full bg-accent opacity-0 shadow-lg transition-opacity group-hover:opacity-100" />
            </div>
          </div>
        </div>

        {/* Controls */}
        <div className="flex items-center">
          {/* Left side - Fixed width */}
          <div className="flex w-[200px] items-center gap-3">
            {/* Time */}
            <div className="font-medium text-sm text-white/70 tabular-nums">
              {formatTime(currentTime)}
            </div>
          </div>

          {/* Center - Playback Controls */}
          <div className="flex flex-1 items-center justify-center gap-2">
            <button
              className="rounded-full p-2 text-white/70 transition-colors hover:bg-white/10 hover:text-white"
              onClick={skipBack}
              title="Skip back 10s"
            >
              <SkipBack size={24} weight="fill" />
            </button>

            <button
              className="rounded-full bg-accent p-3 text-white shadow-lg transition-all hover:scale-105 hover:bg-accent/90"
              onClick={togglePlay}
            >
              {playing ? (
                <Pause size={28} weight="fill" />
              ) : (
                <Play className="translate-x-0.5" size={28} weight="fill" />
              )}
            </button>

            <button
              className="rounded-full p-2 text-white/70 transition-colors hover:bg-white/10 hover:text-white"
              onClick={skipForward}
              title="Skip forward 10s"
            >
              <SkipForward size={24} weight="fill" />
            </button>
          </div>

          {/* Right side - Fixed width matching left */}
          <div className="flex w-[200px] items-center justify-end gap-3">
            {/* Time remaining */}
            <div className="font-medium text-sm text-white/70 tabular-nums">
              -{formatTime(duration - currentTime)}
            </div>

            {/* Volume */}
            <div className="flex items-center gap-2">
              <button
                className="rounded-md p-2 text-white/70 transition-colors hover:bg-white/10 hover:text-white"
                onClick={() => setMuted(!muted)}
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
                  className="h-1 w-full cursor-pointer appearance-none rounded-full bg-white/20 [&::-webkit-slider-thumb]:size-3 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-accent [&::-webkit-slider-thumb]:shadow-lg [&::-webkit-slider-thumb]:transition-all [&::-webkit-slider-thumb]:hover:scale-110"
                  max="1"
                  min="0"
                  onChange={(e) => setVolume(Number.parseFloat(e.target.value))}
                  step="0.01"
                  type="range"
                  value={volume}
                />
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
