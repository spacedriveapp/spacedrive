import clsx from "clsx";
import { File, FileText, Image, FilmStrip, MusicNote, FileCode } from "@phosphor-icons/react";

interface ThumbnailProps {
  src?: string;
  kind?: string;
  name: string;
  size?: "sm" | "md" | "lg";
  className?: string;
}

const iconForKind = (kind?: string) => {
  if (!kind) return File;
  const k = kind.toLowerCase();
  if (k.includes("image") || ["jpg", "png", "gif", "svg"].includes(k))
    return Image;
  if (k.includes("video") || ["mp4", "mov", "avi"].includes(k)) return FilmStrip;
  if (k.includes("audio") || ["mp3", "wav", "flac"].includes(k)) return MusicNote;
  if (["js", "ts", "tsx", "jsx", "py", "rs"].includes(k)) return FileCode;
  if (["pdf", "txt", "md"].includes(k)) return FileText;
  return File;
};

export function Thumbnail({
  src,
  kind,
  name,
  size = "md",
  className,
}: ThumbnailProps) {
  const Icon = iconForKind(kind);

  const sizeClasses = {
    sm: "size-16",
    md: "size-24",
    lg: "size-32",
  };

  const iconSizeClasses = {
    sm: "size-8",
    md: "size-12",
    lg: "size-16",
  };

  return (
    <div
      className={clsx(
        "relative flex items-center justify-center rounded-lg bg-app-box border border-app-line overflow-hidden",
        sizeClasses[size],
        className,
      )}
    >
      {src ? (
        <img
          src={src}
          alt={name}
          className="w-full h-full object-cover"
          loading="lazy"
        />
      ) : (
        <Icon
          className={clsx(iconSizeClasses[size], "text-ink-dull/40")}
          weight="thin"
        />
      )}
    </div>
  );
}