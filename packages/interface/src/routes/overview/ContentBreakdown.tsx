import AudioIcon from "@sd/assets/icons/Audio.png";
import CodeIcon from "@sd/assets/icons/Code-20.png";
import DocumentIcon from "@sd/assets/icons/Document.png";
import FolderIcon from "@sd/assets/icons/Folder.png";
import ImageIcon from "@sd/assets/icons/Image.png";
import VideoIcon from "@sd/assets/icons/Video.png";
import clsx from "clsx";
import { motion } from "framer-motion";

interface ContentBreakdownProps {
  totalFiles: number;
}

// MOCK: Content type breakdown (future feature)
// This would come from query:statistics.content_breakdown
interface ContentType {
  type: string;
  count: number;
  percentage: number;
  color: string;
  icon: string;
}

export function ContentBreakdown({ totalFiles }: ContentBreakdownProps) {
  // MOCK DATA - Simulates content type aggregation
  const mockContentTypes: ContentType[] = [
    {
      type: "Images",
      count: Math.floor(totalFiles * 0.35),
      percentage: 35,
      color: "from-purple-500 to-pink-500",
      icon: ImageIcon,
    },
    {
      type: "Documents",
      count: Math.floor(totalFiles * 0.25),
      percentage: 25,
      color: "from-accent to-cyan-500",
      icon: DocumentIcon,
    },
    {
      type: "Code",
      count: Math.floor(totalFiles * 0.15),
      percentage: 15,
      color: "from-green-500 to-emerald-500",
      icon: CodeIcon,
    },
    {
      type: "Videos",
      count: Math.floor(totalFiles * 0.12),
      percentage: 12,
      color: "from-red-500 to-orange-500",
      icon: VideoIcon,
    },
    {
      type: "Audio",
      count: Math.floor(totalFiles * 0.08),
      percentage: 8,
      color: "from-yellow-500 to-amber-500",
      icon: AudioIcon,
    },
    {
      type: "Other",
      count: Math.floor(totalFiles * 0.05),
      percentage: 5,
      color: "from-gray-500 to-slate-500",
      icon: FolderIcon,
    },
  ];

  return (
    <div className="overflow-hidden rounded-xl border border-app-line bg-app-box">
      <div className="border-app-line border-b px-6 py-4">
        <h2 className="font-semibold text-base text-ink">Content Breakdown</h2>
        <p className="mt-1 text-ink-dull text-sm">
          File types across your library
          <span className="ml-2 rounded-md border border-sidebar-line bg-sidebar-box px-2 py-0.5 font-medium text-sidebar-ink-dull text-xs">
            PREVIEW
          </span>
        </p>
      </div>

      <div className="p-6">
        {/* Horizontal stacked bar */}
        <div className="mb-6">
          <div className="flex h-4 overflow-hidden rounded-full bg-app">
            {mockContentTypes.map((contentType, idx) => (
              <motion.div
                animate={{
                  width: `${contentType.percentage}%`,
                }}
                className={clsx(
                  "h-full bg-gradient-to-r",
                  contentType.color,
                  "first:rounded-l-full last:rounded-r-full"
                )}
                initial={{ width: 0 }}
                key={contentType.type}
                title={`${contentType.type}: ${contentType.count.toLocaleString()} files`}
                transition={{
                  duration: 1,
                  delay: idx * 0.1,
                  ease: "easeOut",
                }}
              />
            ))}
          </div>
        </div>

        {/* Breakdown list */}
        <div className="grid grid-cols-2 gap-3 lg:grid-cols-3">
          {mockContentTypes.map((contentType, idx) => (
            <motion.div
              animate={{ opacity: 1, y: 0 }}
              className="group flex cursor-pointer items-center gap-3 rounded-lg bg-app p-3 transition-colors hover:bg-app-hover"
              initial={{ opacity: 0, y: 10 }}
              key={contentType.type}
              transition={{ delay: idx * 0.05 }}
            >
              {/*<div className="p-2 rounded-lg bg-sidebar-box group-hover:scale-110 transition-transform">*/}
              <img
                alt={contentType.type}
                className="size-8"
                src={contentType.icon}
              />
              {/*</div>*/}

              <div className="min-w-0 flex-1">
                <div className="flex items-baseline gap-2">
                  <span className="font-semibold text-ink text-sm">
                    {contentType.count.toLocaleString()}
                  </span>
                  <span className="text-ink-faint text-xs">
                    {contentType.percentage}%
                  </span>
                </div>
                <div className="truncate text-ink-dull text-xs">
                  {contentType.type}
                </div>
              </div>
            </motion.div>
          ))}
        </div>
      </div>
    </div>
  );
}
