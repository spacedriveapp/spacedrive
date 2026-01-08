import { MagnifyingGlass, Plus } from "@phosphor-icons/react";
import type { Tag } from "@sd/ts-client";
import { Popover, usePopover } from "@sd/ui";
import clsx from "clsx";
import { useEffect, useState } from "react";
import {
  useLibraryMutation,
  useNormalizedQuery,
} from "../../contexts/SpacedriveContext";

interface TagSelectorProps {
  onSelect: (tag: Tag) => void;
  onClose?: () => void;
  contextTags?: Tag[];
  autoFocus?: boolean;
  className?: string;
  /** Optional file ID to apply newly created tags to */
  fileId?: string;
  /** Optional content identity UUID (preferred for content-based tagging) */
  contentId?: string;
}

/**
 * Dropdown menu for searching and selecting tags
 * Features fuzzy search, context-aware suggestions, and keyboard navigation
 */
export function TagSelector({
  onSelect,
  onClose,
  contextTags = [],
  autoFocus = true,
  className,
  fileId,
  contentId,
}: TagSelectorProps) {
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);

  const createTag = useLibraryMutation("tags.create");

  // Fetch all tags using search with empty query
  // Using select to normalize TagSearchResult[] to Tag[] for consistent cache structure
  const { data: allTags = [] } = useNormalizedQuery({
    wireMethod: "query:tags.search",
    input: { query: "" },
    resourceType: "tag",
    select: (data: any) =>
      data?.tags?.map((result: any) => result.tag || result).filter(Boolean) ??
      [],
  });

  // Check if query matches an existing tag
  const exactMatch = allTags.find(
    (tag) => tag.canonical_name.toLowerCase() === query.toLowerCase()
  );

  // Filter tags based on search query
  const filteredTags =
    query.length > 0
      ? allTags.filter(
          (tag) =>
            tag.canonical_name.toLowerCase().includes(query.toLowerCase()) ||
            tag.aliases?.some((alias) =>
              alias.toLowerCase().includes(query.toLowerCase())
            ) ||
            tag.abbreviation?.toLowerCase().includes(query.toLowerCase())
        )
      : allTags;

  // Reset selected index when filtered tags change
  useEffect(() => {
    setSelectedIndex(0);
  }, [filteredTags.length]);

  // Keyboard navigation
  const handleKeyDown = async (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelectedIndex((prev) => Math.min(prev + 1, filteredTags.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelectedIndex((prev) => Math.max(prev - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      // If there's a match, select it
      if (filteredTags[selectedIndex]) {
        handleSelect(filteredTags[selectedIndex]!);
      }
      // If there's text but no match, create new tag
      else if (query.trim().length > 0 && !exactMatch) {
        await handleCreateTag();
      }
    } else if (e.key === "Escape") {
      e.preventDefault();
      onClose?.();
    }
  };

  const handleSelect = (tag: Tag) => {
    onSelect(tag);
    setQuery("");
    onClose?.();
  };

  const handleCreateTag = async () => {
    if (!query.trim()) return;

    try {
      const color = `#${Math.floor(Math.random() * 16_777_215)
        .toString(16)
        .padStart(6, "0")}`;
      const result = await createTag.mutateAsync({
        canonical_name: query.trim(),
        aliases: [],
        color,
        apply_to: contentId
          ? { type: "Content", ids: [contentId] }
          : fileId
            ? { type: "Entry", ids: [Number.parseInt(fileId)] }
            : undefined,
      });

      // Construct a Tag object from the result to pass to onSelect
      // The full tag will be available in the cache shortly via resource events
      const newTag: Tag = {
        id: result.tag_id,
        canonical_name: result.canonical_name,
        display_name: null,
        formal_name: null,
        abbreviation: null,
        aliases: [],
        namespace: result.namespace || null,
        tag_type: "Standard",
        color,
        icon: null,
        description: null,
        is_organizational_anchor: false,
        privacy_level: "Normal",
        search_weight: 0,
        attributes: {},
        composition_rules: [],
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
        created_by_device: result.tag_id, // Placeholder
      };

      onSelect(newTag);
      setQuery("");
      onClose?.();
    } catch (err) {
      console.error("Failed to create tag:", err);
    }
  };

  return (
    <div className={clsx("flex flex-col overflow-hidden", className)}>
      {/* Search Input */}
      <div className="flex items-center gap-2 border-app-line border-b px-3 py-2">
        <MagnifyingGlass className="flex-shrink-0 text-ink-dull" size={16} />
        <input
          autoFocus={autoFocus}
          className="flex-1 bg-transparent text-ink text-sm outline-none placeholder:text-ink-faint"
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Search tags..."
          type="text"
          value={query}
        />
      </div>

      {/* Results */}
      <div className="max-h-64 overflow-y-auto">
        {/* Create new tag option */}
        {query.trim().length > 0 && !exactMatch && (
          <button
            className={clsx(
              "flex w-full items-center gap-2 border-app-line border-b px-3 py-2 text-sm transition-colors",
              selectedIndex === -1
                ? "bg-app-hover text-ink"
                : "text-ink-dull hover:bg-app-hover hover:text-ink"
            )}
            onClick={handleCreateTag}
            onMouseEnter={() => setSelectedIndex(-1)}
          >
            <Plus className="flex-shrink-0" size={16} weight="bold" />
            <span className="flex-1 text-left">
              Create tag "<strong>{query}</strong>"
            </span>
            <kbd className="rounded bg-app-line px-1.5 py-0.5 text-ink-faint text-xs">
              ↵
            </kbd>
          </button>
        )}

        {filteredTags.length === 0 && !query.trim() ? (
          <div className="px-3 py-4 text-center text-ink-dull text-sm">
            No tags yet
          </div>
        ) : filteredTags.length === 0 && query.trim() ? null : (
          filteredTags.map((tag, index) => (
            <button
              className={clsx(
                "flex w-full items-center gap-2 px-3 py-2 text-sm transition-colors",
                index === selectedIndex
                  ? "bg-app-hover text-ink"
                  : "text-ink-dull hover:bg-app-hover hover:text-ink"
              )}
              key={tag.id}
              onClick={() => handleSelect(tag)}
              onMouseEnter={() => setSelectedIndex(index)}
            >
              {/* Color dot */}
              <span
                className="size-2 flex-shrink-0 rounded-full"
                style={{ backgroundColor: tag.color || "#3B82F6" }}
              />

              {/* Tag name */}
              <span className="flex-1 truncate text-left">
                {tag.canonical_name}
              </span>

              {/* Namespace badge */}
              {tag.namespace && (
                <span className="rounded bg-app-line px-1.5 py-0.5 text-ink-faint text-xs">
                  {tag.namespace}
                </span>
              )}
            </button>
          ))
        )}
      </div>
    </div>
  );
}

interface TagSelectorButtonProps {
  onSelect: (tag: Tag) => void;
  trigger: React.ReactNode;
  contextTags?: Tag[];
  /** Optional file ID to apply newly created tags to */
  fileId?: string;
  /** Optional content identity UUID (preferred for content-based tagging) */
  contentId?: string;
}

/**
 * Wrapper component that shows TagSelector in a dropdown when trigger is clicked
 */
export function TagSelectorButton({
  onSelect,
  trigger,
  contextTags,
  fileId,
  contentId,
}: TagSelectorButtonProps) {
  const popover = usePopover();

  return (
    <Popover className="w-64 p-0" popover={popover} trigger={trigger}>
      <TagSelector
        contentId={contentId}
        contextTags={contextTags}
        fileId={fileId}
        onClose={() => popover.setOpen(false)}
        onSelect={(tag) => {
          onSelect(tag);
          popover.setOpen(false);
        }}
      />
    </Popover>
  );
}
