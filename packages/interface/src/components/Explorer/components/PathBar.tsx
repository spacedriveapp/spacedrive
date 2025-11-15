import { useState, useEffect } from "react";
import { motion } from "framer-motion";
import clsx from "clsx";
import { CaretRight } from "@phosphor-icons/react";
import type { SdPath, LibraryDeviceInfo } from "@sd/ts-client/generated/types";
import { sdPathToUri } from "../utils";
import LaptopIcon from "@sd/assets/icons/Laptop.png";

interface PathBarProps {
  path: SdPath;
  devices: Map<string, LibraryDeviceInfo>;
  onNavigate: (path: SdPath) => void;
}

interface PathSegment {
  name: string;
  path: SdPath;
}

function getCurrentDirectoryName(sdPath: SdPath): string {
  if ("Physical" in sdPath) {
    const parts = sdPath.Physical.path.split("/").filter(Boolean);
    return parts[parts.length - 1] || "/";
  }

  if ("Cloud" in sdPath) {
    const parts = sdPath.Cloud.path.split("/").filter(Boolean);
    return parts[parts.length - 1] || sdPath.Cloud.identifier;
  }

  if ("Content" in sdPath) {
    return "Content";
  }

  return "";
}

function parsePathSegments(sdPath: SdPath): PathSegment[] {
  if ("Physical" in sdPath) {
    const { device_slug, path } = sdPath.Physical;
    const parts = path.split("/").filter(Boolean);

    return [
      {
        name: `/`,
        path: {
          Physical: {
            device_slug,
            path: "/",
          },
        },
      },
      ...parts.map((part, index) => ({
        name: part,
        path: {
          Physical: {
            device_slug,
            path: "/" + parts.slice(0, index + 1).join("/"),
          },
        },
      })),
    ];
  }

  if ("Cloud" in sdPath) {
    const { service, identifier, path } = sdPath.Cloud;
    const parts = path.split("/").filter(Boolean);

    return [
      {
        name: identifier,
        path: {
          Cloud: {
            service,
            identifier,
            path: "",
          },
        },
      },
      ...parts.map((part, index) => ({
        name: part,
        path: {
          Cloud: {
            service,
            identifier,
            path: parts.slice(0, index + 1).join("/"),
          },
        },
      })),
    ];
  }

  return [];
}

export function PathBar({ path, devices, onNavigate }: PathBarProps) {
  const [isExpanded, setIsExpanded] = useState(false);
  const [isShiftHeld, setIsShiftHeld] = useState(false);
  const uri = sdPathToUri(path);
  const currentDir = getCurrentDirectoryName(path);
  const segments = parsePathSegments(path);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Shift") setIsShiftHeld(true);
    };
    const handleKeyUp = (e: KeyboardEvent) => {
      if (e.key === "Shift") setIsShiftHeld(false);
    };

    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("keyup", handleKeyUp);

    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("keyup", handleKeyUp);
    };
  }, []);

  const showUri = isExpanded && isShiftHeld;

  // Calculate widths for three states
  const collapsedWidth = currentDir.length * 8.5 + 70;
  const breadcrumbsWidth = Math.min(
    segments.reduce((sum, seg) => sum + seg.name.length * 6.5, 0) +
    (segments.length - 1) * 16 + // separators
    70, // base padding + icon
    600
  );
  const uriWidth = Math.min(uri.length * 7 + 70, 600);

  const currentWidth = !isExpanded ? collapsedWidth : showUri ? uriWidth : breadcrumbsWidth;

  return (
    <motion.div
      animate={{ width: currentWidth }}
      transition={{ duration: 0.2, ease: [0.25, 1, 0.5, 1] }}
      onMouseEnter={() => setIsExpanded(true)}
      onMouseLeave={() => setIsExpanded(false)}
      className={clsx(
        "flex items-center gap-1.5 h-8 px-3 rounded-full",
        "backdrop-blur-xl border border-sidebar-line/30",
        "bg-sidebar-box/20 transition-colors",
        "focus-within:bg-sidebar-box/30 focus-within:border-sidebar-line/40"
      )}
    >
      <img
        src={LaptopIcon}
        alt="Device"
        className="size-5 opacity-60 flex-shrink-0"
      />

      {showUri ? (
        <input
          type="text"
          value={uri}
          readOnly
          className={clsx(
            "bg-transparent border-0 outline-none ring-0 flex-1 min-w-0",
            "text-xs font-medium text-sidebar-ink",
            "placeholder:text-sidebar-inkFaint",
            "select-all cursor-text",
            "focus:ring-0 focus:outline-none"
          )}
          placeholder="No path selected"
        />
      ) : isExpanded ? (
        <div className="flex items-center gap-1 flex-1 min-w-0 overflow-hidden">
          {segments.map((segment, index) => {
            const isLast = index === segments.length - 1;
            return (
              <div key={index} className="flex items-center gap-1 flex-shrink-0">
                <button
                  onClick={() => !isLast && onNavigate(segment.path)}
                  disabled={isLast}
                  className={clsx(
                    "text-xs font-medium transition-colors whitespace-nowrap",
                    isLast
                      ? "text-sidebar-ink cursor-default"
                      : "text-sidebar-inkDull hover:text-sidebar-ink cursor-pointer"
                  )}
                >
                  {segment.name}
                </button>
                {!isLast && (
                  <CaretRight className="size-3 text-sidebar-inkFaint" weight="bold" />
                )}
              </div>
            );
          })}
        </div>
      ) : (
        <input
          type="text"
          value={currentDir}
          readOnly
          className={clsx(
            "bg-transparent border-0 outline-none ring-0 flex-1 min-w-0",
            "text-xs font-medium text-sidebar-ink",
            "placeholder:text-sidebar-inkFaint",
            "select-all cursor-text",
            "focus:ring-0 focus:outline-none"
          )}
          placeholder="No path selected"
        />
      )}
    </motion.div>
  );
}
