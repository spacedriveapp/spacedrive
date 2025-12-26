import { useNavigate } from "react-router-dom";
import { useNormalizedQuery } from "../../context";
import type { ContentKind } from "@sd/ts-client";
import { getIcon } from "@sd/assets/util";

interface ContentKindStat {
	kind: ContentKind;
	name: string;
	file_count: bigint | number;
}

interface ContentKindStatsOutput {
	stats: ContentKindStat[];
	total_files: bigint | number;
}

// Map content kind names to icon names and colors
// Keys must match backend ContentKind variants (lowercase)
// Icon names must match actual files in packages/assets/icons/
const CONTENT_KIND_CONFIG: Record<string, { iconName: string; color: string }> =
	{
		image: { iconName: "Image", color: "#3B82F6" },
		video: { iconName: "Video", color: "#8B5CF6" },
		audio: { iconName: "Audio", color: "#10B981" },
		document: { iconName: "Document", color: "#F59E0B" },
		archive: { iconName: "Archive", color: "#6366F1" },
		code: { iconName: "Text", color: "#EF4444" }, // No Code.png, using Text.png
		text: { iconName: "Text", color: "#6B7280" },
		database: { iconName: "Database", color: "#14B8A6" },
		book: { iconName: "Book", color: "#8B5CF6" },
		font: { iconName: "Text", color: "#F59E0B" },
		mesh: { iconName: "Mesh", color: "#06B6D4" },
		config: { iconName: "Document", color: "#6366F1" },
		encrypted: { iconName: "Encrypted", color: "#DC2626" },
		key: { iconName: "Key", color: "#FCD34D" },
		executable: { iconName: "Executable", color: "#7C3AED" },
		binary: { iconName: "Executable", color: "#6B7280" },
		spreadsheet: { iconName: "Document", color: "#10B981" },
		presentation: { iconName: "Document", color: "#F97316" },
		email: { iconName: "Document", color: "#3B82F6" },
		calendar: { iconName: "Document", color: "#06B6D4" },
		contact: { iconName: "Document", color: "#EC4899" },
		web: { iconName: "Globe", color: "#3B82F6" },
		shortcut: { iconName: "Link", color: "#8B5CF6" },
		package: { iconName: "Package", color: "#F59E0B" },
		model_entry: { iconName: "Mesh", color: "#06B6D4" },
		memory: { iconName: "Database", color: "#6366F1" },
		unknown: { iconName: "Document", color: "#6B7280" },
	};

function formatFileCount(count: number): string {
	if (count >= 1000000) {
		return `${(count / 1000000).toFixed(1)}M`;
	}
	if (count >= 1000) {
		return `${(count / 1000).toFixed(1)}K`;
	}
	return count.toString();
}

/**
 * File Kinds View
 * Shows content kinds (images, videos, audio, etc.) with file counts
 */
export function FileKindsView() {
	const navigate = useNavigate();

	// Fetch content kind statistics
	const { data: statsData, isLoading } = useNormalizedQuery<
		Record<string, never>,
		ContentKindStatsOutput
	>({
		wireMethod: "query:files.content_kind_stats",
		input: {},
		resourceType: "content_kind",
	});

	const stats = (statsData?.stats ?? []).sort(
		(a, b) => Number(b.file_count) - Number(a.file_count),
	);
	const totalFiles = Number(statsData?.total_files ?? 0);

	if (isLoading) {
		return (
			<div className="flex items-center justify-center h-full">
				<span className="text-ink-dull">Loading file kinds...</span>
			</div>
		);
	}

	const handleKindClick = (kind: ContentKind) => {
		// TODO: Navigate to explorer with content kind filter
		// For now, just log
		console.log("Content kind clicked:", kind);
	};

	return (
		<div className="flex flex-col h-full">
			{/* Header */}
			<div className="px-6 py-4 border-b border-app-line">
				<div className="flex items-center justify-between">
					<div>
						<h1 className="text-2xl font-bold text-ink">
							File Kinds
						</h1>
						<p className="text-sm text-ink-dull mt-1">
							Browse your files by content type
						</p>
					</div>
					<div className="text-right">
						<div className="text-2xl font-bold text-ink">
							{formatFileCount(totalFiles)}
						</div>
						<div className="text-xs text-ink-dull">Total Files</div>
					</div>
				</div>
			</div>

			{/* Content Grid */}
			<div className="flex-1 overflow-auto p-3">
				<div
					className="grid"
					style={{
						gridTemplateColumns:
							"repeat(auto-fill, minmax(140px, 1fr))",
						gap: "8px",
					}}
				>
					{stats.map((stat) => {
						const config =
							CONTENT_KIND_CONFIG[stat.name] ||
							CONTENT_KIND_CONFIG.unknown;
						const icon = getIcon(
							config.iconName,
							true,
							null,
							false,
						);

						return (
							<button
								key={stat.name}
								onClick={() => handleKindClick(stat.kind)}
								className="flex flex-col items-center justify-center p-4 rounded-lg hover:bg-app-box/50 transition-colors group aspect-square"
							>
								<img
									src={icon}
									alt={stat.name}
									className="w-16 h-16 mb-3"
								/>
								<div className="text-center w-full">
									<div className="text-sm font-medium text-ink capitalize mb-1">
										{stat.name}
									</div>
									<div className="text-xs text-ink-dull">
										{formatFileCount(
											Number(stat.file_count),
										)}{" "}
										{Number(stat.file_count) === 1
											? "file"
											: "files"}
									</div>
								</div>
							</button>
						);
					})}
				</div>
			</div>
		</div>
	);
}
