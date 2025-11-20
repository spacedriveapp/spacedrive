import type { File, ContentKind } from "@sd/ts-client/generated/types";
import { File as FileComponent } from "../Explorer/File";
import { formatBytes } from "../Explorer/utils";
import { usePlatform } from "../../platform";
import { useState, useEffect, useRef } from "react";
import {
  MagnifyingGlassPlus,
  MagnifyingGlassMinus,
  ArrowCounterClockwise,
} from "@phosphor-icons/react";
import { VideoPlayer } from "./VideoPlayer";
import { AudioPlayer } from "./AudioPlayer";
import { useZoomPan } from "./useZoomPan";
import { Folder } from "@sd/assets/icons";

interface ContentRendererProps {
  file: File;
}

function ImageRenderer({ file }: ContentRendererProps) {
  const platform = usePlatform();
  const containerRef = useRef<HTMLDivElement>(null);
  const [originalLoaded, setOriginalLoaded] = useState(false);
  const [originalUrl, setOriginalUrl] = useState<string | null>(null);
  const { zoom, zoomIn, zoomOut, reset, transform } = useZoomPan(containerRef);

  useEffect(() => {
    if (!platform.convertFileSrc) {
      return;
    }

    const sdPath = file.sd_path as any;
    const physicalPath = sdPath?.Physical?.path;

    if (!physicalPath) {
      console.log(
        "[ImageRenderer] No physical path available, sd_path:",
        file.sd_path,
      );
      return;
    }

    const url = platform.convertFileSrc(physicalPath);
    console.log(
      "[ImageRenderer] Loading original from:",
      physicalPath,
      "-> URL:",
      url,
    );
    setOriginalUrl(url);
  }, [file, platform]);

  // Get highest resolution thumbnail first
  const getHighestResThumbnail = () => {
    const thumbnails = file.sidecars?.filter((s) => s.kind === "thumb") || [];
    if (thumbnails.length === 0) return null;

    const highest = thumbnails.sort((a, b) => {
      const aSize = parseInt(
        a.variant.split("x")[0]?.replace(/\D/g, "") || "0",
      );
      const bSize = parseInt(
        b.variant.split("x")[0]?.replace(/\D/g, "") || "0",
      );
      return bSize - aSize;
    })[0];

    const serverUrl = (window as any).__SPACEDRIVE_SERVER_URL__;
    const libraryId = (window as any).__SPACEDRIVE_LIBRARY_ID__;
    const contentUuid = file.content_identity?.uuid;

    if (!serverUrl || !libraryId || !contentUuid) return null;

    return `${serverUrl}/sidecar/${libraryId}/${contentUuid}/${highest.kind}/${highest.variant}.${highest.format}`;
  };

  const thumbnailUrl = getHighestResThumbnail();

  return (
    <div
      ref={containerRef}
      className="relative w-full h-full overflow-hidden flex items-center justify-center"
    >
      {/* Zoom Controls */}
      <div className="absolute top-4 right-4 z-10 flex flex-col gap-2">
        <button
          onClick={zoomIn}
          className="rounded-lg bg-app-box/80 p-2 text-ink backdrop-blur-xl transition-colors hover:bg-app-hover"
          title="Zoom in (+)"
        >
          <MagnifyingGlassPlus size={20} weight="bold" />
        </button>
        <button
          onClick={zoomOut}
          className="rounded-lg bg-app-box/80 p-2 text-ink backdrop-blur-xl transition-colors hover:bg-app-hover"
          title="Zoom out (-)"
        >
          <MagnifyingGlassMinus size={20} weight="bold" />
        </button>
        {zoom > 1 && (
          <button
            onClick={reset}
            className="rounded-lg bg-app-box/80 p-2 text-ink backdrop-blur-xl transition-colors hover:bg-app-hover"
            title="Reset zoom (0)"
          >
            <ArrowCounterClockwise size={20} weight="bold" />
          </button>
        )}
      </div>

      {/* Zoom level indicator */}
      {zoom > 1 && (
        <div className="absolute top-4 left-4 z-10 rounded-lg bg-app-box/80 px-3 py-1.5 text-sm font-medium text-ink backdrop-blur-xl">
          {Math.round(zoom * 100)}%
        </div>
      )}

      {/* Image container with zoom/pan transform */}
      <div
        className="relative w-full h-full flex items-center justify-center"
        style={transform}
      >
        {/* High-res thumbnail (loads fast, shows immediately) */}
        {thumbnailUrl && (
          <img
            src={thumbnailUrl}
            alt={file.name}
            className="w-full h-full object-contain"
            style={{
              opacity: originalLoaded ? 0 : 1,
              transition: "opacity 0.3s",
            }}
            draggable={false}
          />
        )}

        {/* Original image (loads async, fades in when ready) */}
        {originalUrl && (
          <img
            src={originalUrl}
            alt={file.name}
            className="absolute inset-0 w-full h-full object-contain"
            style={{
              opacity: originalLoaded ? 1 : 0,
              transition: "opacity 0.3s",
            }}
            onLoad={() => setOriginalLoaded(true)}
            onError={(e) =>
              console.error("[ImageRenderer] Original failed to load:", e)
            }
            draggable={false}
          />
        )}
      </div>
    </div>
  );
}

function VideoRenderer({ file }: ContentRendererProps) {
  const platform = usePlatform();
  const [videoUrl, setVideoUrl] = useState<string | null>(null);

  useEffect(() => {
    if (!platform.convertFileSrc) {
      return;
    }

    const sdPath = file.sd_path as any;
    const physicalPath = sdPath?.Physical?.path;

    if (!physicalPath) {
      console.log("[VideoRenderer] No physical path available");
      return;
    }

    const url = platform.convertFileSrc(physicalPath);
    console.log(
      "[VideoRenderer] Loading video from:",
      physicalPath,
      "-> URL:",
      url,
    );
    setVideoUrl(url);
  }, [file, platform]);

  if (!videoUrl) {
    return (
      <div className="w-full h-full flex items-center justify-center">
        <FileComponent.Thumb
          file={file}
          size={800}
          className="max-w-full max-h-full"
        />
      </div>
    );
  }

  return <VideoPlayer src={videoUrl} file={file} />;
}

function AudioRenderer({ file }: ContentRendererProps) {
  const platform = usePlatform();
  const [audioUrl, setAudioUrl] = useState<string | null>(null);

  useEffect(() => {
    if (!platform.convertFileSrc) {
      return;
    }

    const sdPath = file.sd_path as any;
    const physicalPath = sdPath?.Physical?.path;

    if (!physicalPath) {
      console.log("[AudioRenderer] No physical path available");
      return;
    }

    const url = platform.convertFileSrc(physicalPath);
    console.log(
      "[AudioRenderer] Loading audio from:",
      physicalPath,
      "-> URL:",
      url,
    );
    setAudioUrl(url);
  }, [file, platform]);

  if (!audioUrl) {
    return (
      <div className="w-full h-full flex items-center justify-center">
        <div className="text-center">
          <FileComponent.Thumb file={file} size={200} />
          <div className="mt-6 text-ink text-lg font-medium">{file.name}</div>
          <div className="text-ink-dull text-sm mt-2">Loading...</div>
        </div>
      </div>
    );
  }

  return <AudioPlayer src={audioUrl} file={file} />;
}

function DocumentRenderer({ file }: ContentRendererProps) {
  return (
    <div className="w-full h-full flex items-center justify-center">
      <div className="text-center">
        <FileComponent.Thumb file={file} size={200} />
        <div className="mt-6 text-ink text-lg font-medium">{file.name}</div>
        <div className="text-ink-dull text-sm mt-2 capitalize">
          {file.content_kind}
        </div>
        <div className="text-ink-dull text-xs mt-1">
          {formatBytes(file.size || 0)}
        </div>
      </div>
    </div>
  );
}

function TextRenderer({ file }: ContentRendererProps) {
  // TODO: Load actual text content
  return (
    <div className="w-full h-full flex items-center justify-center">
      <div className="text-center max-w-xl">
        <FileComponent.Thumb file={file} size={120} />
        <div className="mt-4 text-ink text-lg font-medium">{file.name}</div>
        <div className="text-ink-dull text-sm mt-2">Text File</div>
        <div className="text-ink-dull text-xs mt-1">
          {formatBytes(file.size || 0)}
        </div>
        <div className="mt-4 text-xs text-ink-dull">
          Full text preview coming soon
        </div>
      </div>
    </div>
  );
}

function DefaultRenderer({ file }: ContentRendererProps) {
  return (
    <div className="w-full h-full flex items-center justify-center">
      <div className="text-center">
        <FileComponent.Thumb file={file} size={200} />
        <div className="mt-6 text-ink text-lg font-medium">{file.name}</div>
        <div className="text-ink-dull text-sm mt-2 capitalize">
          {file.content_kind}
        </div>
        <div className="text-ink-dull text-xs mt-1">
          {formatBytes(file.size || 0)}
        </div>
      </div>
    </div>
  );
}

export function ContentRenderer({ file }: ContentRendererProps) {
  // Handle directories first
  if (file.kind === "Directory") {
    return (
      <div className="flex flex-col items-center justify-center h-full text-ink-dull">
        <img src={Folder} alt="Folder Icon" className="w-16 h-16 mb-4" />
        <div className="text-lg font-medium text-ink">{file.name}</div>
        <div className="text-sm mt-2">Folder</div>
        {file.size > 0 && (
          <div className="text-xs mt-1">{formatBytes(file.size)}</div>
        )}
      </div>
    );
  }

  const kind = file.content_kind;

  switch (kind) {
    case "image":
      return <ImageRenderer file={file} />;
    case "video":
      return <VideoRenderer file={file} />;
    case "audio":
      return <AudioRenderer file={file} />;
    case "document":
    case "book":
    case "spreadsheet":
    case "presentation":
      return <DocumentRenderer file={file} />;
    case "text":
    case "code":
    case "config":
      return <TextRenderer file={file} />;
    default:
      return <DefaultRenderer file={file} />;
  }
}
