import { Tag as TagIcon } from "@phosphor-icons/react";
import type { Tag } from "@sd/ts-client";
import { Button } from "@sd/ui";
import clsx from "clsx";
import { AnimatePresence, motion } from "framer-motion";
import { useState } from "react";
import {
  useLibraryMutation,
  useNormalizedQuery,
} from "../../contexts/SpacedriveContext";
import { useKeybind } from "../../hooks/useKeybind";
import { useSelection } from "./SelectionContext";

interface TagAssignmentModeProps {
  isActive: boolean;
  onExit: () => void;
}

/**
 * Tag Assignment Mode - Quick keyboard-driven tagging
 *
 * Features:
 * - Toggle tags with number keys (1-9, 0)
 * - Switch palettes with Cmd+Shift+[1-9]
 * - Visual feedback for applied tags
 * - Works on selected files
 */
export function TagAssignmentMode({
  isActive,
  onExit,
}: TagAssignmentModeProps) {
  const { selectedFiles } = useSelection();
  const [currentPaletteIndex, setCurrentPaletteIndex] = useState(0);

  const applyTag = useLibraryMutation("tags.apply");

  // Fetch all tags (for now, we'll use the first 10 as the default palette)
  // TODO: Implement user-defined palettes
  const { data: tagsData } = useNormalizedQuery<
    { query: string },
    { tags: Array<{ tag: Tag } | Tag> }
  >({
    wireMethod: "query:tags.search",
    input: { query: "" },
    resourceType: "tag",
  });

  // Extract tags from search results
  // Handle both wrapped format ({ tag, relevance }) from initial query
  // and raw Tag objects from real-time ResourceChanged events
  const allTags =
    tagsData?.tags?.map((result) => ("tag" in result ? result.tag : result)) ??
    [];
  const paletteTags = allTags.slice(0, 10) as Tag[];

  // Keyboard shortcuts using keybind registry
  useKeybind("explorer.exitTagMode", onExit, { enabled: isActive });
  useKeybind("explorer.toggleTag1", () => handleToggleTag(0), {
    enabled: isActive,
  });
  useKeybind("explorer.toggleTag2", () => handleToggleTag(1), {
    enabled: isActive,
  });
  useKeybind("explorer.toggleTag3", () => handleToggleTag(2), {
    enabled: isActive,
  });
  useKeybind("explorer.toggleTag4", () => handleToggleTag(3), {
    enabled: isActive,
  });
  useKeybind("explorer.toggleTag5", () => handleToggleTag(4), {
    enabled: isActive,
  });
  useKeybind("explorer.toggleTag6", () => handleToggleTag(5), {
    enabled: isActive,
  });
  useKeybind("explorer.toggleTag7", () => handleToggleTag(6), {
    enabled: isActive,
  });
  useKeybind("explorer.toggleTag8", () => handleToggleTag(7), {
    enabled: isActive,
  });
  useKeybind("explorer.toggleTag9", () => handleToggleTag(8), {
    enabled: isActive,
  });
  useKeybind("explorer.toggleTag10", () => handleToggleTag(9), {
    enabled: isActive,
  });

  const handleToggleTag = async (index: number) => {
    const tag = paletteTags[index];
    if (!tag || selectedFiles.length === 0) return;

    // Get content IDs from selected files (filter out files without content identity)
    const contentIds = selectedFiles
      .map((f) => f.content_identity?.uuid)
      .filter((id): id is string => id != null);

    if (contentIds.length === 0) return;

    try {
      await applyTag.mutateAsync({
        targets: { type: "Content", ids: contentIds },
        tag_ids: [tag.id],
      });
    } catch (err) {
      console.error("Failed to apply tag:", err);
    }
  };

  // Check if a tag is active (all selected files have it)
  const isTagActive = (tag: Tag) => {
    if (selectedFiles.length === 0) return false;
    return selectedFiles.every((file) =>
      file.tags?.some((t) => t.id === tag.id)
    );
  };

  if (!isActive) return null;

  return (
    <AnimatePresence>
      <motion.div
        animate={{ y: 0, opacity: 1 }}
        className="absolute right-1 bottom-2 left-1 z-50"
        exit={{ y: 100, opacity: 0 }}
        initial={{ y: 100, opacity: 0 }}
        transition={{ duration: 0.2, ease: [0.25, 1, 0.5, 1] }}
      >
        <div className="rounded-xl border border-sidebar-line/50 bg-sidebar/80 px-4 py-3 shadow-lg backdrop-blur-xl">
          <div className="flex items-center gap-3">
            {/* Mode Label */}
            <div className="flex items-center gap-2">
              <TagIcon className="text-accent" size={16} weight="bold" />
              <span className="font-semibold text-sidebar-ink text-sm">
                Tag Mode
              </span>
              {selectedFiles.length > 0 && (
                <span className="text-sidebar-inkDull text-xs">
                  {selectedFiles.length}{" "}
                  {selectedFiles.length === 1 ? "item" : "items"}
                </span>
              )}
            </div>

            {/* Palette Tags */}
            <div className="flex flex-1 gap-1.5">
              {paletteTags.map((tag, index) => {
                const active = isTagActive(tag);
                const number = index === 9 ? 0 : index + 1;

                return (
                  <button
                    className={clsx(
                      "inline-flex items-center gap-2 rounded-md px-2.5 py-1 font-medium text-sm transition-all",
                      active ? "scale-105 shadow-md" : "hover:scale-105"
                    )}
                    key={tag.id}
                    onClick={() => handleToggleTag(index)}
                    style={{
                      backgroundColor: active
                        ? `${tag.color || "#3B82F6"}40`
                        : `${tag.color || "#3B82F6"}20`,
                      color: tag.color || "#3B82F6",
                    }}
                  >
                    {/* Keyboard Number */}
                    <kbd className="min-w-[16px] rounded bg-black/20 px-1 py-0.5 text-center font-bold text-[10px]">
                      {number}
                    </kbd>

                    {/* Tag Name */}
                    <span className="max-w-[120px] truncate">
                      {tag.canonical_name}
                    </span>

                    {/* Active Checkmark */}
                    {active && <span className="text-xs">✓</span>}
                  </button>
                );
              })}
            </div>

            {/* Exit Button */}
            <Button onClick={onExit} size="sm" variant="accent">
              Done
            </Button>
          </div>

          {/* Help Text */}
          {selectedFiles.length === 0 && (
            <div className="mt-2 text-center text-sidebar-inkFaint text-xs">
              Select files to start tagging • Press 1-9/0 to toggle tags • Esc
              to exit
            </div>
          )}
        </div>
      </motion.div>
    </AnimatePresence>
  );
}
