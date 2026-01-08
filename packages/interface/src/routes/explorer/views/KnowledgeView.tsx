import {
  Chat,
  Database,
  File as FileIcon,
  FileText,
  FilmStrip,
  Folder,
  Image,
  MusicNote,
  Sparkle,
  Tag as TagIcon,
} from "@phosphor-icons/react";
import type { ContentKind, File } from "@sd/ts-client";
import clsx from "clsx";
import { useMemo } from "react";
import { KnowledgeInspector } from "../../../components/Inspector/variants/KnowledgeInspector";
import { useNormalizedQuery } from "../../../contexts/SpacedriveContext";
import { useExplorer } from "../context";
import { File as FileComponent } from "../File";
import { getContentKind } from "../utils";

const CONTENT_KIND_ICONS: Record<ContentKind, React.ElementType> = {
  image: Image,
  video: FilmStrip,
  audio: MusicNote,
  document: FileText,
  archive: Folder,
  code: FileText,
  text: FileText,
  database: Database,
  book: FileText,
  font: FileText,
  mesh: FileIcon,
  config: FileText,
  encrypted: FileIcon,
  key: FileIcon,
  executable: FileIcon,
  binary: FileIcon,
  spreadsheet: FileText,
  presentation: FileText,
  email: FileText,
  calendar: FileText,
  contact: FileText,
  web: FileText,
  shortcut: FileIcon,
  package: Folder,
  model_entry: FileIcon,
  unknown: FileIcon,
};

const CONTENT_KIND_LABELS: Record<ContentKind, string> = {
  image: "Images",
  video: "Videos",
  audio: "Audio",
  document: "Documents",
  archive: "Archives",
  code: "Code",
  text: "Text",
  database: "Databases",
  book: "Books",
  font: "Fonts",
  mesh: "3D Models",
  config: "Config",
  encrypted: "Encrypted",
  key: "Keys",
  executable: "Apps",
  binary: "Binary",
  spreadsheet: "Spreadsheets",
  presentation: "Presentations",
  email: "Emails",
  calendar: "Calendar",
  contact: "Contacts",
  web: "Web",
  shortcut: "Shortcuts",
  package: "Packages",
  model_entry: "Models",
  unknown: "Other",
};

export function KnowledgeView() {
  const { inspectorVisible, currentPath, sortBy, viewSettings } = useExplorer();

  const directoryQuery = useNormalizedQuery({
    wireMethod: "query:files.directory_listing",
    input: currentPath
      ? {
          path: currentPath,
          limit: null,
          include_hidden: false,
          sort_by: sortBy,
          folders_first: viewSettings.foldersFirst,
        }
      : null,
    resourceType: "file",
    enabled: !!currentPath,
  });

  const files = (directoryQuery.data?.files || []) as File[];

  // Group files by content kind
  const filesByKind = useMemo(() => {
    const groups = new Map<ContentKind, File[]>();

    files.forEach((file) => {
      const kind = getContentKind(file) || "unknown";
      if (!groups.has(kind)) {
        groups.set(kind, []);
      }
      groups.get(kind)!.push(file);
    });

    // Sort by count and return top categories
    return Array.from(groups.entries())
      .sort((a, b) => b[1].length - a[1].length)
      .slice(0, 6);
  }, [files]);

  // Collect all unique tags
  const allTags = useMemo(() => {
    const tagMap = new Map<
      string,
      { id: string; name: string; color: string; count: number }
    >();

    files.forEach((file) => {
      file.tags?.forEach((tag) => {
        if (tagMap.has(tag.id)) {
          tagMap.get(tag.id)!.count++;
        } else {
          tagMap.set(tag.id, {
            id: tag.id,
            name: tag.canonical_name,
            color: tag.color || "#3B82F6",
            count: 1,
          });
        }
      });
    });

    return Array.from(tagMap.values()).sort((a, b) => b.count - a.count);
  }, [files]);

  return (
    <div className="flex h-full gap-2">
      {/* Main content area */}
      <div className="no-scrollbar flex-1 overflow-y-auto px-6 py-4">
        <div className="max-w-5xl space-y-6">
          {/* Header */}
          <div className="flex items-center gap-3">
            <Sparkle className="size-8 text-accent" weight="fill" />
            <div>
              <h1 className="font-semibold text-2xl text-ink">
                Knowledge View
              </h1>
              <p className="text-ink-dull text-sm">
                AI-powered insights for {files.length} items
              </p>
            </div>
          </div>

          {/* Content Piles */}
          <Section icon={Folder} title="Content">
            <div className="grid grid-cols-2 gap-4 md:grid-cols-3 lg:grid-cols-6">
              {filesByKind.map(([kind, kindFiles]) => (
                <ContentPile
                  files={kindFiles.slice(0, 3)}
                  key={kind}
                  kind={kind}
                  totalCount={kindFiles.length}
                />
              ))}
            </div>
          </Section>

          {/* Tags */}
          {allTags.length > 0 && (
            <Section icon={TagIcon} title="Tags">
              <div className="flex flex-wrap gap-2">
                {allTags.map((tag) => (
                  <button
                    className="flex items-center gap-2 rounded-full border border-app-line bg-app-box px-3 py-1.5 transition-colors hover:bg-app-hover"
                    key={tag.id}
                  >
                    <div
                      className="size-2 rounded-full"
                      style={{
                        backgroundColor: tag.color,
                      }}
                    />
                    <span className="font-medium text-ink text-xs">
                      {tag.name}
                    </span>
                    <span className="text-ink-dull text-xs">({tag.count})</span>
                  </button>
                ))}
              </div>
            </Section>
          )}

          {/* Summary & Conversations */}
          <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
            {/* Summary */}
            <Section icon={Sparkle} title="Summary">
              <div className="space-y-2 text-ink-dull text-sm">
                <p>
                  This directory contains {files.length} items across{" "}
                  {filesByKind.length} content types.
                </p>
                {filesByKind.length > 0 && (
                  <p>
                    Most common type: {CONTENT_KIND_LABELS[filesByKind[0][0]]} (
                    {filesByKind[0][1].length} items)
                  </p>
                )}
                {allTags.length > 0 && (
                  <p>
                    Tagged items:{" "}
                    {allTags.reduce((sum, tag) => sum + tag.count, 0)}
                  </p>
                )}
              </div>
            </Section>

            {/* Conversations */}
            <Section icon={Chat} title="Conversations">
              <div className="grid grid-cols-2 gap-2">
                <ConversationCard
                  preview="Can you help sort these by date?"
                  time="2h ago"
                  title="Organize photos"
                />
                <ConversationCard
                  preview="Looking for duplicate files in..."
                  time="Yesterday"
                  title="Find duplicates"
                />
              </div>
            </Section>
          </div>

          {/* Intelligence Sidecars */}
          <Section icon={Database} title="Intelligence">
            <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
              <SidecarItem
                kind="OCR Text"
                size="2.4 MB"
                status="ready"
                variant="Extracted text from 12 images"
              />
              <SidecarItem
                kind="Thumbnails"
                size="8.1 MB"
                status="ready"
                variant="Generated for 48 media files"
              />
              <SidecarItem
                kind="Video Transcripts"
                size="—"
                status="pending"
                variant="Speech-to-text from 3 videos"
              />
              <SidecarItem
                kind="Embeddings"
                size="14.2 MB"
                status="ready"
                variant="Semantic vectors for search"
              />
            </div>
          </Section>
        </div>
      </div>

      {/* Dedicated Knowledge Inspector */}
      {inspectorVisible && (
        <div className="h-full w-96 shrink-0 pr-2 pb-2">
          <div className="h-full overflow-hidden rounded-lg bg-sidebar/65">
            <KnowledgeInspector />
          </div>
        </div>
      )}
    </div>
  );
}

function Section({
  title,
  icon: Icon,
  children,
}: {
  title: string;
  icon: React.ElementType;
  children: React.ReactNode;
}) {
  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2">
        <Icon className="size-4 text-ink-dull" weight="bold" />
        <h2 className="font-semibold text-ink text-sm">{title}</h2>
      </div>
      {children}
    </div>
  );
}

function ContentPile({
  kind,
  files,
  totalCount,
}: {
  kind: ContentKind;
  files: File[];
  totalCount: number;
}) {
  const Icon = CONTENT_KIND_ICONS[kind];
  const label = CONTENT_KIND_LABELS[kind];

  return (
    <button className="group flex flex-col items-center gap-2 rounded-lg p-3 transition-colors hover:bg-app-box/40">
      {/* Stacked file previews */}
      <div className="relative aspect-square w-full">
        {files.length > 0 ? (
          files.map((file, i) => (
            <div
              className="absolute inset-0"
              key={file.id}
              style={{
                transform: `rotate(${(i - 1) * 3}deg) translateY(${i * 2}px)`,
                zIndex: files.length - i,
              }}
            >
              <FileComponent.Thumb
                className="h-full w-full rounded-md shadow-sm"
                file={file}
                iconScale={0.5}
                size={120}
              />
            </div>
          ))
        ) : (
          <div className="flex h-full w-full items-center justify-center">
            <Icon className="size-12 text-ink-faint" weight="thin" />
          </div>
        )}
      </div>

      {/* Label */}
      <div className="text-center">
        <div className="font-medium text-ink text-xs">{label}</div>
        <div className="text-[10px] text-ink-dull">{totalCount} items</div>
      </div>
    </button>
  );
}

function ConversationCard({
  title,
  preview,
  time,
}: {
  title: string;
  preview: string;
  time: string;
}) {
  return (
    <button className="flex flex-col gap-1.5 rounded-lg border border-app-line/50 bg-app-box/40 p-3 text-left transition-colors hover:border-app-line hover:bg-app-box">
      <div className="flex items-start justify-between gap-2">
        <div className="truncate font-medium text-ink text-xs">{title}</div>
        <Sparkle className="size-3 shrink-0 text-accent" weight="fill" />
      </div>
      <p className="line-clamp-2 text-[11px] text-ink-dull">{preview}</p>
      <span className="text-[10px] text-ink-faint">{time}</span>
    </button>
  );
}

function SidecarItem({
  kind,
  variant,
  status,
  size,
}: {
  kind: string;
  variant: string;
  status: "ready" | "pending";
  size: string;
}) {
  return (
    <div className="flex items-start gap-3 rounded-lg border border-app-line/50 bg-app-box/40 p-3">
      <div className="flex size-10 shrink-0 items-center justify-center rounded-md border border-accent/20 bg-accent/10">
        <Database className="size-5 text-accent" weight="bold" />
      </div>
      <div className="min-w-0 flex-1">
        <div className="font-medium text-ink text-xs">{kind}</div>
        <div className="text-[11px] text-ink-dull">{variant}</div>
        <div className="mt-1 text-[10px] text-ink-faint">{size}</div>
      </div>
      <span
        className={clsx(
          "shrink-0 rounded-full px-2 py-0.5 font-semibold text-[10px]",
          status === "ready" && "bg-accent/20 text-accent",
          status === "pending" && "bg-ink-faint/20 text-ink-dull"
        )}
      >
        {status}
      </span>
    </div>
  );
}
