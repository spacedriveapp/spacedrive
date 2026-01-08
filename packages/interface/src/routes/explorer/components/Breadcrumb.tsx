import type { SdPath } from "@sd/ts-client";
import clsx from "clsx";

interface BreadcrumbProps {
  path: SdPath;
  onNavigate: (path: SdPath) => void;
}

interface PathSegment {
  name: string;
  path: SdPath;
}

function parseSdPathSegments(sdPath: SdPath): PathSegment[] {
  if ("Physical" in sdPath) {
    const { device_slug, path } = sdPath.Physical;
    const parts = path.split("/").filter(Boolean);

    return parts.map((part, index) => ({
      name: part,
      path: {
        Physical: {
          device_slug,
          path: "/" + parts.slice(0, index + 1).join("/"),
        },
      },
    }));
  }

  if ("Cloud" in sdPath) {
    const { service, identifier, path } = sdPath.Cloud;
    const parts = path.split("/").filter(Boolean);

    return parts.map((part, index) => ({
      name: part,
      path: {
        Cloud: {
          service,
          identifier,
          path: parts.slice(0, index + 1).join("/"),
        },
      },
    }));
  }

  return [];
}

export function Breadcrumb({ path, onNavigate }: BreadcrumbProps) {
  const segments = parseSdPathSegments(path);

  return (
    <div className="flex items-center gap-1 text-sm">
      {segments.map((segment, index) => {
        const isLast = index === segments.length - 1;
        return (
          <div className="flex items-center gap-1" key={index}>
            {index > 0 && <span className="text-ink-faint">/</span>}
            <button
              className={clsx(
                isLast
                  ? "cursor-default font-medium text-ink"
                  : "cursor-pointer text-ink-dull hover:text-ink"
              )}
              disabled={isLast}
              onClick={() => !isLast && onNavigate(segment.path)}
            >
              {segment.name}
            </button>
          </div>
        );
      })}
    </div>
  );
}
