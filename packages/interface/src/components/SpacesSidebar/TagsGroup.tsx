import { CaretRight, Plus, Tag as TagIcon } from "@phosphor-icons/react";
import type { Tag } from "@sd/ts-client";
import clsx from "clsx";
import { useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  useLibraryMutation,
  useNormalizedQuery,
} from "../../contexts/SpacedriveContext";
import { useExplorer } from "../../routes/explorer/context";
import { GroupHeader } from "./GroupHeader";

interface TagsGroupProps {
  isCollapsed: boolean;
  onToggle: () => void;
  sortableAttributes?: any;
  sortableListeners?: any;
}

interface TagItemProps {
  tag: Tag;
  depth?: number;
}

function TagItem({ tag, depth = 0 }: TagItemProps) {
  const navigate = useNavigate();
  const { loadPreferencesForSpaceItem } = useExplorer();
  const [isExpanded, setIsExpanded] = useState(false);

  // TODO: Fetch children when hierarchy is implemented
  const children: Tag[] = [];
  const hasChildren = children.length > 0;

  const handleClick = () => {
    loadPreferencesForSpaceItem(`tag:${tag.id}`);
    navigate(`/tag/${tag.id}`);
  };

  return (
    <div>
      <button
        className={clsx(
          "flex w-full items-center gap-2 rounded-lg px-2 py-1.5 font-medium text-sidebar-ink-dull text-sm transition-colors hover:bg-sidebar-box hover:text-sidebar-ink",
          tag.privacy_level === "Archive" && "opacity-50",
          tag.privacy_level === "Hidden" && "opacity-25"
        )}
        onClick={handleClick}
        style={{ paddingLeft: `${8 + depth * 12}px` }}
      >
        {/* Expand/Collapse for children */}
        {hasChildren && (
          <CaretRight
            className={clsx(
              "flex-shrink-0 transition-transform",
              isExpanded && "rotate-90"
            )}
            onClick={(e) => {
              e.stopPropagation();
              setIsExpanded(!isExpanded);
            }}
            size={10}
            weight="bold"
          />
        )}

        {/* Color dot or icon */}
        {tag.icon ? (
          <TagIcon
            size={16}
            style={{ color: tag.color || "#3B82F6" }}
            weight="bold"
          />
        ) : (
          <span
            className="size-2 flex-shrink-0 rounded-full"
            style={{ backgroundColor: tag.color || "#3B82F6" }}
          />
        )}

        {/* Tag name */}
        <span className="flex-1 truncate text-left">{tag.canonical_name}</span>

        {/* File count badge (if available) */}
        {/* TODO: Add file count when available from backend */}
      </button>

      {/* Children (recursive) */}
      {isExpanded &&
        children.map((child) => (
          <TagItem depth={depth + 1} key={child.id} tag={child} />
        ))}
    </div>
  );
}

export function TagsGroup({
  isCollapsed,
  onToggle,
  sortableAttributes,
  sortableListeners,
}: TagsGroupProps) {
  const navigate = useNavigate();
  const { loadPreferencesForSpaceItem } = useExplorer();
  const [isCreating, setIsCreating] = useState(false);
  const [newTagName, setNewTagName] = useState("");

  const createTag = useLibraryMutation("tags.create");

  // Fetch tags with real-time updates using search with empty query
  // Using select to normalize TagSearchResult[] to Tag[] for consistent cache structure
  const { data: tags = [], isLoading } = useNormalizedQuery({
    wireMethod: "query:tags.search",
    input: { query: "" },
    resourceType: "tag",
    select: (data: any) =>
      data?.tags?.map((result: any) => result.tag || result).filter(Boolean) ??
      [],
  });

  const handleCreateTag = async () => {
    if (!newTagName.trim()) return;

    try {
      const result = await createTag.mutateAsync({
        canonical_name: newTagName.trim(),
        display_name: null,
        formal_name: null,
        abbreviation: null,
        aliases: [],
        namespace: null,
        tag_type: null,
        color: `#${Math.floor(Math.random() * 16_777_215)
          .toString(16)
          .padStart(6, "0")}`,
        icon: null,
        description: null,
        is_organizational_anchor: null,
        privacy_level: null,
        search_weight: null,
        attributes: null,
        apply_to: null,
      });

      // Navigate to the new tag
      if (result?.tag_id) {
        loadPreferencesForSpaceItem(`tag:${result.tag_id}`);
        navigate(`/tag/${result.tag_id}`);
      }

      setNewTagName("");
      setIsCreating(false);
    } catch (err) {
      console.error("Failed to create tag:", err);
    }
  };

  return (
    <div>
      <GroupHeader
        isCollapsed={isCollapsed}
        label="Tags"
        onToggle={onToggle}
        rightComponent={
          tags.length > 0 && (
            <span className="ml-auto text-sidebar-ink-faint">
              {tags.length}
            </span>
          )
        }
        sortableAttributes={sortableAttributes}
        sortableListeners={sortableListeners}
      />

      {/* Items */}
      {!isCollapsed && (
        <div className="space-y-0.5">
          {isLoading ? (
            <div className="px-2 py-1 text-sidebar-ink-faint text-xs">
              Loading...
            </div>
          ) : tags.length === 0 ? (
            <div className="px-2 py-1 text-sidebar-ink-faint text-xs">
              No tags yet
            </div>
          ) : (
            tags.map((tag) => <TagItem key={tag.id} tag={tag} />)
          )}

          {/* Create Tag Button/Input */}
          {isCreating ? (
            <div className="px-2 py-1.5">
              <input
                autoFocus
                className="w-full rounded-md border border-sidebar-line bg-sidebar-box px-2 py-1 text-sidebar-ink text-xs outline-none placeholder:text-sidebar-ink-faint focus:border-accent"
                onBlur={() => {
                  if (!newTagName.trim()) {
                    setIsCreating(false);
                  }
                }}
                onChange={(e) => setNewTagName(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    handleCreateTag();
                  } else if (e.key === "Escape") {
                    setIsCreating(false);
                    setNewTagName("");
                  }
                }}
                placeholder="Tag name..."
                type="text"
                value={newTagName}
              />
            </div>
          ) : (
            <button
              className="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 font-medium text-sidebar-ink-dull text-xs transition-colors hover:bg-sidebar-box hover:text-sidebar-ink"
              onClick={() => setIsCreating(true)}
            >
              <Plus size={12} weight="bold" />
              <span>New Tag</span>
            </button>
          )}
        </div>
      )}
    </div>
  );
}
