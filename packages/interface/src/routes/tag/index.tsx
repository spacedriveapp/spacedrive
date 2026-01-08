import { CaretRight, Funnel } from "@phosphor-icons/react";
import { Fragment } from "react";
import { useParams } from "react-router-dom";
import { useNormalizedQuery } from "../../contexts/SpacedriveContext";
import { ExplorerView } from "../explorer/ExplorerView";

/**
 * Tag Explorer View
 * Shows all files tagged with a specific tag, with hierarchy awareness and filtering
 */
export function TagView() {
  const { tagId } = useParams<{ tagId: string }>();

  // Fetch the tag details
  const { data: tagData, isLoading: tagLoading } = useNormalizedQuery({
    wireMethod: "query:tags.by_id",
    input: { tag_id: tagId },
    resourceType: "tag",
    resourceId: tagId,
    enabled: !!tagId,
  });

  // Fetch tag ancestors for breadcrumb
  const { data: ancestorsData } = useNormalizedQuery({
    wireMethod: "query:tags.ancestors",
    input: { tag_id: tagId },
    resourceType: "tag",
    resourceId: tagId,
    enabled: !!tagId,
  });

  // Fetch tag children for quick filters
  const { data: childrenData } = useNormalizedQuery({
    wireMethod: "query:tags.children",
    input: { tag_id: tagId },
    resourceType: "tag",
    resourceId: tagId,
    enabled: !!tagId,
  });

  // Fetch related tags for suggestions
  const { data: relatedData } = useNormalizedQuery({
    wireMethod: "query:tags.related",
    input: { tag_id: tagId },
    resourceType: "tag",
    resourceId: tagId,
    enabled: !!tagId,
  });

  // Fetch files with this tag
  const { data: filesData, isLoading: filesLoading } = useNormalizedQuery({
    wireMethod: "query:files.by_tag",
    input: {
      tag_id: tagId,
      include_children: false, // TODO: Make this toggleable
      min_confidence: 0.0,
    },
    resourceType: "file",
    enabled: !!tagId,
  });

  const tag = tagData?.tag;
  const ancestors = ancestorsData?.ancestors ?? [];
  const children = childrenData?.children ?? [];
  const related = relatedData?.related ?? [];
  const files = filesData?.files ?? [];

  if (tagLoading) {
    return (
      <div className="flex h-full items-center justify-center">
        <span className="text-ink-dull">Loading tag...</span>
      </div>
    );
  }

  if (!tag) {
    return (
      <div className="flex h-full items-center justify-center">
        <span className="text-ink-dull">Tag not found</span>
      </div>
    );
  }

  return (
    <div className="flex h-full">
      {/* Main Content */}
      <div className="flex flex-1 flex-col">
        {/* Header */}
        <div className="space-y-3 border-app-line border-b px-4 py-3">
          {/* Breadcrumb */}
          <div className="flex items-center gap-2 text-sm">
            {ancestors.map((ancestor, i) => (
              <Fragment key={ancestor.id}>
                <button
                  className="font-medium text-ink-dull transition-colors hover:text-ink"
                  onClick={() => (window.location.href = `/tag/${ancestor.id}`)}
                >
                  {ancestor.canonical_name}
                </button>
                <CaretRight className="text-ink-faint" size={12} />
              </Fragment>
            ))}
            <div className="flex items-center gap-2">
              {tag.icon ? (
                <span style={{ color: tag.color || "#3B82F6" }}>
                  {/* TODO: Render icon */}
                </span>
              ) : (
                <span
                  className="size-3 rounded-full"
                  style={{ backgroundColor: tag.color || "#3B82F6" }}
                />
              )}
              <span className="font-semibold text-ink">
                {tag.canonical_name}
              </span>
            </div>
          </div>

          {/* Options Row */}
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              {/* TODO: Add filters button */}
              <button className="flex items-center gap-2 rounded-md border border-app-line bg-app-box px-3 py-1.5 text-sm transition-colors hover:bg-app-hover">
                <Funnel size={14} />
                <span>Filters</span>
              </button>
            </div>

            {/* File Count */}
            <span className="text-ink-dull text-sm">
              {filesLoading
                ? "Loading..."
                : `${files.length} ${files.length === 1 ? "file" : "files"}`}
            </span>
          </div>

          {/* Child Tag Quick Filters */}
          {children.length > 0 && (
            <div className="flex flex-wrap items-center gap-2">
              <span className="font-semibold text-ink-dull text-xs">
                Children:
              </span>
              {children.map((child) => (
                <button
                  className="inline-flex items-center gap-1.5 rounded-md border border-app-line bg-app-box px-2 py-1 font-medium text-xs transition-colors hover:bg-app-hover"
                  key={child.id}
                  onClick={() => (window.location.href = `/tag/${child.id}`)}
                  style={{ color: child.color || "#3B82F6" }}
                >
                  <span
                    className="size-1.5 rounded-full"
                    style={{ backgroundColor: child.color || "#3B82F6" }}
                  />
                  {child.canonical_name}
                </button>
              ))}
            </div>
          )}
        </div>

        {/* Explorer View */}
        <div className="flex-1 overflow-auto">
          {filesLoading ? (
            <div className="flex h-full items-center justify-center">
              <span className="text-ink-dull">Loading files...</span>
            </div>
          ) : files.length === 0 ? (
            <div className="flex h-full flex-col items-center justify-center gap-2">
              <span className="text-ink-dull">No files with this tag</span>
              <span className="text-ink-faint text-xs">
                Files will appear here when you tag them
              </span>
            </div>
          ) : (
            <ExplorerView />
          )}
        </div>
      </div>

      {/* Sidebar: Related Tags */}
      {related.length > 0 && (
        <aside className="w-64 space-y-4 overflow-y-auto border-app-line border-l p-4">
          <div>
            <h4 className="mb-2 font-semibold text-ink-dull text-sm">
              Related Tags
            </h4>
            <div className="space-y-1">
              {related.map((relatedTag) => (
                <button
                  className="flex w-full items-center justify-between rounded-md px-2 py-1.5 text-sm transition-colors hover:bg-app-hover"
                  key={relatedTag.id}
                  onClick={() =>
                    (window.location.href = `/tag/${relatedTag.id}`)
                  }
                >
                  <div className="flex items-center gap-2">
                    <span
                      className="size-2 rounded-full"
                      style={{ backgroundColor: relatedTag.color || "#3B82F6" }}
                    />
                    <span className="text-ink">
                      {relatedTag.canonical_name}
                    </span>
                  </div>
                  {relatedTag.co_occurrence_count && (
                    <span className="text-ink-faint text-xs">
                      {relatedTag.co_occurrence_count}
                    </span>
                  )}
                </button>
              ))}
            </div>
          </div>
        </aside>
      )}
    </div>
  );
}
