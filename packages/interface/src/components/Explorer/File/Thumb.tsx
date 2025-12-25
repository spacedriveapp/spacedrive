import { useState, memo, useEffect } from "react";
import clsx from "clsx";
import { getIcon, getBeardedIcon, beardedIconUrls } from "@sd/assets/util";
import type { File } from "@sd/ts-client";
import { ThumbstripScrubber } from "./ThumbstripScrubber";
import { getContentKind } from "../utils";
import { useServer } from "../../../ServerContext";
import { getVirtualMetadata } from "../utils/virtualFiles";

interface ThumbProps {
  file: File;
  size?: number;
  className?: string;
  frameClassName?: string; // Custom frame styling (border, radius, bg)
  iconScale?: number; // Scale factor for fallback icon (0-1, default 1)
  squareMode?: boolean; // Whether thumbnail is cropped to square (media view) or maintains aspect ratio
}

// Global cache for thumbnail loaded states (survives component unmount/remount)
const thumbLoadedCache = new Map<string, boolean>();
const thumbErrorCache = new Map<string, boolean>();

export const Thumb = memo(function Thumb({
  file,
  size = 100,
  className,
  frameClassName,
  iconScale = 1,
  squareMode = false,
}: ThumbProps) {
  const cacheKey = `${file.id}-${size}`;
  const { buildSidecarUrl } = useServer();

  const [thumbLoaded, setThumbLoaded] = useState(
    () => thumbLoadedCache.get(cacheKey) || false,
  );
  const [thumbError, setThumbError] = useState(
    () => thumbErrorCache.get(cacheKey) || false,
  );

  // Update cache when state changes
  useEffect(() => {
    if (thumbLoaded) thumbLoadedCache.set(cacheKey, true);
  }, [thumbLoaded, cacheKey]);

  useEffect(() => {
    if (thumbError) thumbErrorCache.set(cacheKey, true);
  }, [thumbError, cacheKey]);

  const iconSize = size * iconScale;

  // Check for virtual file icon override
  const virtualMetadata = getVirtualMetadata(file);
  const iconOverride = virtualMetadata?.iconUrl;

  // Check if this is a video with thumbstrip sidecar
  const isVideo = getContentKind(file) === "video";
  const hasThumbstrip = file.sidecars?.some((s) => s.kind === "thumbstrip");

  // Get appropriate thumbnail URL from sidecars based on size
  const getThumbnailUrl = (targetSize: number) => {
    // Need content_identity to build sidecar URL
    if (!file.content_identity?.uuid) {
      return null;
    }

    // Find thumbnail sidecar closest to requested size
    const thumbnails = file.sidecars.filter((s) => s.kind === "thumb");

    if (thumbnails.length === 0) {
      return null;
    }

    // Prefer 1x (lower resolution) variants for better performance
    // Only use higher resolution for very large sizes (>400px)
    const preferredSize = targetSize <= 400 ? targetSize * 0.6 : targetSize;

    const thumbnail = thumbnails.sort((a, b) => {
      // Parse variant (e.g., "grid@1x", "detail@1x") to get size and scale
      const aSize = parseInt(
        a.variant.split("x")[0]?.replace(/\D/g, "") || "0",
      );
      const bSize = parseInt(
        b.variant.split("x")[0]?.replace(/\D/g, "") || "0",
      );

      // Extract scale factor (1x, 2x, 3x) from variants like "grid@1x" or "detail@2x"
      const aScaleMatch = a.variant.match(/@(\d+)x/);
      const bScaleMatch = b.variant.match(/@(\d+)x/);
      const aScale = aScaleMatch ? parseInt(aScaleMatch[1]) : 1;
      const bScale = bScaleMatch ? parseInt(bScaleMatch[1]) : 1;

      // Strongly prefer 1x variants (add penalty for higher scales)
      const aPenalty = (aScale - 1) * 100;
      const bPenalty = (bScale - 1) * 100;

      // Find closest match to preferred size, with scale penalty
      return (
        Math.abs(aSize - preferredSize) +
        aPenalty -
        (Math.abs(bSize - preferredSize) + bPenalty)
      );
    })[0];

    return buildSidecarUrl(
      file.content_identity.uuid,
      thumbnail.kind,
      thumbnail.variant,
      thumbnail.format,
    );
  };

  const thumbnailSrc = getThumbnailUrl(size);

  // Get content kind (prefers content_identity.kind, falls back to content_kind)
  const contentKind = getContentKind(file);
  const fileKind =
    contentKind && contentKind !== "unknown"
      ? contentKind
      : file.kind === "File"
        ? file.extension || "File"
        : file.kind;
  const kindCapitalized = fileKind.charAt(0).toUpperCase() + fileKind.slice(1);

  // Use icon override from virtual files (devices, volumes), otherwise use default icon logic
  const icon =
    iconOverride ||
    getIcon(
      kindCapitalized,
      true, // Dark theme
      file.extension,
      file.kind === "Directory",
    );

  // Check if using generic Document icon (not a Spacedrive variant like Document_pdf)
  const genericDocumentIcon = getIcon("Document", true, null, false);
  const isUsingGenericIcon = icon === genericDocumentIcon;

  // Get bearded icon for extension overlay
  const beardedIconName = getBeardedIcon(file.extension, file.name);
  const beardedIconUrl = beardedIconName
    ? beardedIconUrls[beardedIconName]
    : null;

  // Below 60px, show only bearded icon at full size; above, show as overlay at 40%
  const smallIconThreshold = 60;
  const isSmallIcon = size < smallIconThreshold;
  const badgeSize = isSmallIcon ? iconSize : iconSize * 0.4;

  // Only show bearded badge if using generic Document icon (not Spacedrive variants)
  const showBeardedBadge =
    beardedIconUrl &&
    file.kind === "File" &&
    isUsingGenericIcon &&
    (contentKind === "code" ||
      contentKind === "document" ||
      contentKind === "config");

  return (
    <div
      className={clsx(
        "relative pointer-events-none flex shrink-0 grow-0 items-center justify-center",
        className,
      )}
      style={{
        width: size,
        height: size,
        minWidth: size,
        minHeight: size,
        maxWidth: size,
        maxHeight: size,
      }}
    >
      {/* Always show icon first (instant), then thumbnail loads over it */}
      {/* Hide document icon if small and showing bearded badge */}
      {!(isSmallIcon && showBeardedBadge) && (
        <img
          src={icon}
          alt=""
          className={clsx(
            "object-contain transition-opacity",
            // Only hide icon if we actually have a thumbnail that loaded
            thumbLoaded && thumbnailSrc && "opacity-0",
          )}
          style={{
            width: iconSize,
            height: iconSize,
            maxWidth: "100%",
            maxHeight: "100%",
          }}
        />
      )}

      {/* Load thumbnail if available */}
      {thumbnailSrc && !thumbError && (
        <img
          src={thumbnailSrc}
          alt={file.name}
          className={clsx(
            "absolute inset-0 m-auto max-h-full max-w-full object-contain transition-opacity",
            // Default frame styling (can be overridden)
            frameClassName ||
              "rounded-lg border border-app-line/50 bg-app-box/30",
            !thumbLoaded && "opacity-0",
          )}
          onLoad={() => setThumbLoaded(true)}
          onError={() => setThumbError(true)}
        />
      )}

      {/* Bearded icon badge overlay (centered, slightly toward bottom) */}
      {showBeardedBadge && beardedIconUrl && (
        <img
          src={beardedIconUrl}
          alt=""
          className="absolute left-1/2 top-[55%] -translate-x-1/2 -translate-y-1/2"
          style={{
            width: badgeSize,
            height: badgeSize,
          }}
        />
      )}

      {/* Thumbstrip scrubber overlay (for videos with thumbstrips) */}
      {isVideo && hasThumbstrip && thumbLoaded && (
        <ThumbstripScrubber file={file} size={size} squareMode={squareMode} />
      )}
    </div>
  );
});

export function Icon({
  file,
  size = 24,
  className,
}: {
  file: File;
  size?: number;
  className?: string;
}) {
  // Check for virtual file icon override
  const virtualMetadata = getVirtualMetadata(file);
  const iconOverride = virtualMetadata?.iconUrl;

  // Get content kind (prefers content_identity.kind, falls back to content_kind)
  const contentKind = getContentKind(file);
  const fileKind =
    contentKind && contentKind !== "unknown"
      ? contentKind
      : file.kind === "File"
        ? file.extension || "File"
        : file.kind;
  const kindCapitalized = fileKind.charAt(0).toUpperCase() + fileKind.slice(1);

  // Use icon override from virtual files (devices, volumes), otherwise use default icon logic
  const icon =
    iconOverride ||
    getIcon(
      kindCapitalized,
      true, // Dark theme
      file.extension,
      file.kind === "Directory",
    );

  return (
    <img
      src={icon}
      alt=""
      className={className}
      style={{ width: size, height: size }}
    />
  );
}
