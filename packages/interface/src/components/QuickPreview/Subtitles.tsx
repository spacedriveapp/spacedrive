import type { File } from "@sd/ts-client";
import { useEffect, useState } from "react";
import { useServer } from "../../contexts/ServerContext";

interface SubtitleCue {
  index: number;
  startTime: number;
  endTime: number;
  text: string;
}

export interface SubtitleSettings {
  fontSize: number; // 0.8 to 2.0
  position: "bottom" | "top";
  backgroundOpacity: number; // 0 to 1
}

interface SubtitlesProps {
  file: File;
  videoElement: HTMLVideoElement | null;
  settings?: SubtitleSettings;
}

const DEFAULT_SETTINGS: SubtitleSettings = {
  fontSize: 1.5,
  position: "bottom",
  backgroundOpacity: 0.9,
};

/**
 * Parse SRT (SubRip) subtitle format
 * Format:
 * 1
 * 00:00:01,000 --> 00:00:04,000
 * Subtitle text here
 *
 * 2
 * 00:00:05,000 --> 00:00:08,000
 * Next subtitle
 */
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

export function Subtitles({
  file,
  videoElement,
  settings = DEFAULT_SETTINGS,
}: SubtitlesProps) {
  const [cues, setCues] = useState<SubtitleCue[]>([]);
  const [currentCue, setCurrentCue] = useState<SubtitleCue | null>(null);
  const { buildSidecarUrl } = useServer();

  // Load SRT sidecar if available
  useEffect(() => {
    const srtSidecar = file.sidecars?.find(
      (s) => s.kind === "transcript" && s.variant === "srt"
    );

    if (!(srtSidecar && file.content_identity?.uuid)) {
      return;
    }

    // Map "text" format to "txt" extension (DB stores "text", file is .txt)
    const extension = srtSidecar.format === "text" ? "txt" : srtSidecar.format;
    const srtUrl = buildSidecarUrl(
      file.content_identity.uuid,
      srtSidecar.kind,
      srtSidecar.variant,
      extension
    );

    if (!srtUrl) {
      console.warn("[Subtitles] Server URL or Library ID not available");
      return;
    }

    console.log("[Subtitles] Loading SRT from:", srtUrl);

    fetch(srtUrl)
      .then(async (res) => {
        if (!res.ok) {
          if (res.status === 404) {
            console.log(
              "[Subtitles] No subtitle file found (not generated yet)"
            );
          } else {
            console.error(
              "[Subtitles] Failed to fetch SRT, status:",
              res.status
            );
          }
          return null;
        }
        return res.text();
      })
      .then((srtContent) => {
        if (!srtContent) return;
        const parsed = parseSRT(srtContent);
        console.log(
          "[Subtitles] Loaded and parsed",
          parsed.length,
          "subtitle cues"
        );
        setCues(parsed);
      })
      .catch((err) => {
        console.log("[Subtitles] Subtitles not available:", err.message);
      });
  }, [file, buildSidecarUrl]);

  // Sync with video playback
  useEffect(() => {
    if (!videoElement || cues.length === 0) {
      console.log(
        "[Subtitles] Not setting up sync - videoElement:",
        !!videoElement,
        "cues:",
        cues.length
      );
      return;
    }

    console.log("[Subtitles] Setting up video sync with", cues.length, "cues");

    const updateSubtitle = () => {
      const currentTime = videoElement.currentTime;
      const activeCue = cues.find(
        (cue) => currentTime >= cue.startTime && currentTime <= cue.endTime
      );

      if (activeCue !== currentCue) {
        setCurrentCue(activeCue || null);
      }
    };

    // Update on time change
    videoElement.addEventListener("timeupdate", updateSubtitle);

    // Also update when seeking
    videoElement.addEventListener("seeked", updateSubtitle);

    return () => {
      videoElement.removeEventListener("timeupdate", updateSubtitle);
      videoElement.removeEventListener("seeked", updateSubtitle);
    };
  }, [videoElement, cues, currentCue]);

  if (!currentCue) {
    return null;
  }

  const positionClass = settings.position === "top" ? "top-16" : "bottom-16";

  return (
    <div
      className={`pointer-events-none absolute ${positionClass} right-0 left-0 flex justify-center px-8`}
    >
      <div
        className="max-w-4xl rounded-lg px-6 py-3 text-center backdrop-blur-sm"
        style={{
          backgroundColor: `rgba(0, 0, 0, ${settings.backgroundOpacity})`,
        }}
      >
        <p
          className="text-white leading-relaxed"
          style={{
            fontSize: `${settings.fontSize}rem`,
          }}
        >
          {currentCue.text}
        </p>
      </div>
    </div>
  );
}
