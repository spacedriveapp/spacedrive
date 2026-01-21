import {
	Sparkle,
	Tag as TagIcon,
	Chat,
	Database,
	FilmStrip,
	Image,
	MusicNote,
	File as FileIcon,
	Folder,
	FileText,
} from "@phosphor-icons/react";
import { KnowledgeInspector } from "../../../components/Inspector/variants/KnowledgeInspector";
import { useExplorer } from "../context";
import { useNormalizedQuery } from "../../../contexts/SpacedriveContext";
import type { File, ContentKind } from "@sd/ts-client";
import { getContentKind } from "@sd/ts-client";
import { useMemo } from "react";
import clsx from "clsx";
import { File as FileComponent } from "../File";

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
	const { inspectorVisible, currentPath, sortBy, viewSettings } =
		useExplorer();

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
			<div className="flex-1 overflow-y-auto no-scrollbar px-6 py-4">
				<div className="max-w-5xl space-y-6">
					{/* Header */}
					<div className="flex items-center gap-3">
						<Sparkle className="size-8 text-accent" weight="fill" />
						<div>
							<h1 className="text-2xl font-semibold text-ink">
								Knowledge View
							</h1>
							<p className="text-sm text-ink-dull">
								AI-powered insights for {files.length} items
							</p>
						</div>
					</div>

					{/* Content Piles */}
					<Section title="Content" icon={Folder}>
						<div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-4">
							{filesByKind.map(([kind, kindFiles]) => (
								<ContentPile
									key={kind}
									kind={kind}
									files={kindFiles.slice(0, 3)}
									totalCount={kindFiles.length}
								/>
							))}
						</div>
					</Section>

					{/* Tags */}
					{allTags.length > 0 && (
						<Section title="Tags" icon={TagIcon}>
							<div className="flex flex-wrap gap-2">
								{allTags.map((tag) => (
									<button
										key={tag.id}
										className="flex items-center gap-2 px-3 py-1.5 rounded-full bg-app-box hover:bg-app-hover border border-app-line transition-colors"
									>
										<div
											className="size-2 rounded-full"
											style={{
												backgroundColor: tag.color,
											}}
										/>
										<span className="text-xs font-medium text-ink">
											{tag.name}
										</span>
										<span className="text-xs text-ink-dull">
											({tag.count})
										</span>
									</button>
								))}
							</div>
						</Section>
					)}

					{/* Summary & Conversations */}
					<div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
						{/* Summary */}
						<Section title="Summary" icon={Sparkle}>
							<div className="space-y-2 text-sm text-ink-dull">
								<p>
									This directory contains {files.length} items
									across {filesByKind.length} content types.
								</p>
								{filesByKind.length > 0 && (
									<p>
										Most common type:{" "}
										{CONTENT_KIND_LABELS[filesByKind[0][0]]}{" "}
										({filesByKind[0][1].length} items)
									</p>
								)}
								{allTags.length > 0 && (
									<p>
										Tagged items:{" "}
										{allTags.reduce(
											(sum, tag) => sum + tag.count,
											0,
										)}
									</p>
								)}
							</div>
						</Section>

						{/* Conversations */}
						<Section title="Conversations" icon={Chat}>
							<div className="grid grid-cols-2 gap-2">
								<ConversationCard
									title="Organize photos"
									preview="Can you help sort these by date?"
									time="2h ago"
								/>
								<ConversationCard
									title="Find duplicates"
									preview="Looking for duplicate files in..."
									time="Yesterday"
								/>
							</div>
						</Section>
					</div>

					{/* Intelligence Sidecars */}
					<Section title="Intelligence" icon={Database}>
						<div className="grid grid-cols-1 md:grid-cols-2 gap-3">
							<SidecarItem
								kind="OCR Text"
								variant="Extracted text from 12 images"
								status="ready"
								size="2.4 MB"
							/>
							<SidecarItem
								kind="Thumbnails"
								variant="Generated for 48 media files"
								status="ready"
								size="8.1 MB"
							/>
							<SidecarItem
								kind="Video Transcripts"
								variant="Speech-to-text from 3 videos"
								status="pending"
								size="—"
							/>
							<SidecarItem
								kind="Embeddings"
								variant="Semantic vectors for search"
								status="ready"
								size="14.2 MB"
							/>
						</div>
					</Section>
				</div>
			</div>

			{/* Dedicated Knowledge Inspector */}
			{inspectorVisible && (
				<div className="w-96 h-full shrink-0 pr-2 pb-2">
					<div className="h-full rounded-lg overflow-hidden bg-sidebar/65">
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
				<h2 className="text-sm font-semibold text-ink">{title}</h2>
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
		<button className="group flex flex-col items-center gap-2 p-3 rounded-lg hover:bg-app-box/40 transition-colors">
			{/* Stacked file previews */}
			<div className="relative w-full aspect-square">
				{files.length > 0 ? (
					files.map((file, i) => (
						<div
							key={file.id}
							className="absolute inset-0"
							style={{
								transform: `rotate(${(i - 1) * 3}deg) translateY(${i * 2}px)`,
								zIndex: files.length - i,
							}}
						>
							<FileComponent.Thumb
								file={file}
								size={120}
								iconScale={0.5}
								className="w-full h-full rounded-md shadow-sm"
							/>
						</div>
					))
				) : (
					<div className="flex items-center justify-center w-full h-full">
						<Icon
							className="size-12 text-ink-faint"
							weight="thin"
						/>
					</div>
				)}
			</div>

			{/* Label */}
			<div className="text-center">
				<div className="text-xs font-medium text-ink">{label}</div>
				<div className="text-[10px] text-ink-dull">
					{totalCount} items
				</div>
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
		<button className="flex flex-col gap-1.5 p-3 rounded-lg bg-app-box/40 hover:bg-app-box border border-app-line/50 hover:border-app-line transition-colors text-left">
			<div className="flex items-start justify-between gap-2">
				<div className="text-xs font-medium text-ink truncate">
					{title}
				</div>
				<Sparkle
					className="size-3 text-accent shrink-0"
					weight="fill"
				/>
			</div>
			<p className="text-[11px] text-ink-dull line-clamp-2">{preview}</p>
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
		<div className="flex items-start gap-3 p-3 rounded-lg bg-app-box/40 border border-app-line/50">
			<div className="size-10 shrink-0 rounded-md bg-accent/10 border border-accent/20 flex items-center justify-center">
				<Database className="size-5 text-accent" weight="bold" />
			</div>
			<div className="flex-1 min-w-0">
				<div className="text-xs font-medium text-ink">{kind}</div>
				<div className="text-[11px] text-ink-dull">{variant}</div>
				<div className="text-[10px] text-ink-faint mt-1">{size}</div>
			</div>
			<span
				className={clsx(
					"text-[10px] font-semibold px-2 py-0.5 rounded-full shrink-0",
					status === "ready" && "bg-accent/20 text-accent",
					status === "pending" && "bg-ink-faint/20 text-ink-dull",
				)}
			>
				{status}
			</span>
		</div>
	);
}