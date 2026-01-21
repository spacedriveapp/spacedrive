import {
	Files,
	Tag as TagIcon,
	Calendar,
	HardDrive,
	Folder,
} from "@phosphor-icons/react";
import { useMemo } from "react";
import { InfoRow, Section, Divider, Tag } from "../Inspector";
import clsx from "clsx";
import type { File } from "@sd/ts-client";
import { getContentKind } from "@sd/ts-client";
import { formatBytes } from "../../../routes/explorer/utils";
import { File as FileComponent } from "../../../routes/explorer/File";

interface MultiFileInspectorProps {
	files: File[];
}

export function MultiFileInspector({ files }: MultiFileInspectorProps) {
	// Get last 3 files for thumbnail stacking (v1 style)
	const thumbnailFiles = useMemo(() => {
		return files.slice(-3).reverse();
	}, [files]);

	// Calculate aggregated metadata
	const aggregatedData = useMemo(() => {
		const totalSize = files.reduce((sum, file) => sum + (file.size || 0), 0);

		// Group by content kind
		const kindCounts = new Map<string, number>();
		files.forEach((file) => {
			const kind = getContentKind(file) || "unknown";
			kindCounts.set(kind, (kindCounts.get(kind) || 0) + 1);
		});

		// Get all tags with counts
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

		// Calculate date ranges
		const dates = {
			created: files
				.map((f) => f.created_at)
				.filter(Boolean)
				.sort(),
			modified: files
				.map((f) => f.modified_at)
				.filter(Boolean)
				.sort(),
		};

		return {
			totalSize,
			kindCounts: Array.from(kindCounts.entries()).sort(
				(a, b) => b[1] - a[1],
			),
			tags: Array.from(tagMap.values()).sort((a, b) => b.count - a.count),
			dateRanges: {
				created:
					dates.created.length > 0
						? {
								earliest: dates.created[0],
								latest: dates.created[dates.created.length - 1],
							}
						: null,
				modified:
					dates.modified.length > 0
						? {
								earliest: dates.modified[0],
								latest: dates.modified[dates.modified.length - 1],
							}
						: null,
			},
		};
	}, [files]);

	return (
		<div className="no-scrollbar mask-fade-out flex flex-col space-y-5 overflow-x-hidden overflow-y-scroll pb-10">
			{/* Stacked thumbnails (v1 style) */}
			<div className="px-2 pb-4">
				<div className="relative w-full aspect-square flex items-center justify-center">
					{thumbnailFiles.map((file, i, thumbs) => (
						<div
							key={file.id}
							className={clsx(
								thumbs.length > 1 && "!absolute",
								i === 0 &&
									thumbs.length > 1 &&
									"z-30 !h-[76%] !w-[76%]",
								i === 1 && "z-20 !h-4/5 !w-4/5 rotate-[-5deg]",
								i === 2 && "z-10 !h-[84%] !w-[84%] rotate-[7deg]",
							)}
						>
							<FileComponent.Thumb
								file={file}
								size={thumbs.length === 1 ? 240 : 180}
								className={clsx(
									"w-full h-full rounded-lg",
									thumbs.length > 1 &&
										"shadow-md shadow-black/20",
								)}
							/>
						</div>
					))}
				</div>
			</div>

			{/* File count header */}
			<div className="px-2 pb-3">
				<div className="text-center">
					<h2 className="text-lg font-semibold text-sidebar-ink">
						{files.length} Items Selected
					</h2>
				</div>
			</div>

			<Divider />

			{/* Summary section */}
			<Section title="Summary" icon={Files}>
				<InfoRow label="Total Size" value={formatBytes(aggregatedData.totalSize)} />
				<InfoRow label="Items" value={files.length} />
			</Section>

			{/* File types breakdown */}
			{aggregatedData.kindCounts.length > 0 && (
				<Section title="Types" icon={Folder}>
					{aggregatedData.kindCounts.slice(0, 5).map(([kind, count]) => (
						<InfoRow
							key={kind}
							label={kind.charAt(0).toUpperCase() + kind.slice(1)}
							value={count}
						/>
					))}
				</Section>
			)}

			{/* Tags with opacity based on coverage */}
			{aggregatedData.tags.length > 0 && (
				<Section title="Tags" icon={TagIcon}>
					<div className="flex flex-wrap gap-1.5">
						{aggregatedData.tags.map((tag) => {
							const coverage = tag.count / files.length;
							const opacity = coverage === 1 ? 1 : 0.5;

							return (
								<div
									key={tag.id}
									style={{ opacity }}
									className="transition-opacity"
								>
									<Tag color={tag.color}>
										{tag.name}
										{coverage < 1 && (
											<span className="ml-1 text-[10px]">
												({tag.count})
											</span>
										)}
									</Tag>
								</div>
							);
						})}
					</div>
				</Section>
			)}

			{/* Date ranges */}
			{(aggregatedData.dateRanges.created ||
				aggregatedData.dateRanges.modified) && (
				<Section title="Dates" icon={Calendar}>
					{aggregatedData.dateRanges.created && (
						<InfoRow
							label="Created"
							value={
								aggregatedData.dateRanges.created.earliest ===
								aggregatedData.dateRanges.created.latest
									? new Date(
											aggregatedData.dateRanges.created.earliest,
										).toLocaleDateString()
									: `${new Date(
											aggregatedData.dateRanges.created.earliest,
										).toLocaleDateString()} - ${new Date(
											aggregatedData.dateRanges.created.latest,
										).toLocaleDateString()}`
							}
						/>
					)}
					{aggregatedData.dateRanges.modified && (
						<InfoRow
							label="Modified"
							value={
								aggregatedData.dateRanges.modified.earliest ===
								aggregatedData.dateRanges.modified.latest
									? new Date(
											aggregatedData.dateRanges.modified.earliest,
										).toLocaleDateString()
									: `${new Date(
											aggregatedData.dateRanges.modified.earliest,
										).toLocaleDateString()} - ${new Date(
											aggregatedData.dateRanges.modified.latest,
										).toLocaleDateString()}`
							}
						/>
					)}
				</Section>
			)}
		</div>
	);
}