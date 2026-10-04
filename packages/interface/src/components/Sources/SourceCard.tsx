import type { SourceInfo } from "@sd/ts-client";
import { useNavigate } from "react-router-dom";
import { useAdapterIcons } from "../../hooks/useAdapterIcons";
import { SourceTypeIcon } from "./SourceTypeIcon";
import { SourceStatusBadge } from "./SourceStatusBadge";

interface SourceCardProps {
	source: SourceInfo;
}

function formatRelative(iso: string): string {
	const date = new Date(iso);
	const now = new Date();
	const diffMs = now.getTime() - date.getTime();
	const diffMin = Math.floor(diffMs / 60000);
	const diffHr = Math.floor(diffMin / 60);
	const diffDay = Math.floor(diffHr / 24);

	if (diffMin < 1) return "just now";
	if (diffMin < 60) return `${diffMin}m ago`;
	if (diffHr < 24) return `${diffHr}h ago`;
	if (diffDay < 30) return `${diffDay}d ago`;
	return date.toLocaleDateString();
}

export function SourceCard({ source }: SourceCardProps) {
	const navigate = useNavigate();
	const { getIcon } = useAdapterIcons();

	return (
		<button
			onClick={() => navigate(`/sources/${source.id}`)}
			className="border-app-line bg-app-box hover:border-app-line/80 hover:bg-app-hover group relative rounded-lg border p-4 text-left transition-all"
		>
			<div className="mb-3 flex items-center gap-3">
				<SourceTypeIcon
					type={source.data_type}
					svg={source.adapter_id ? getIcon(source.adapter_id) : null}
					size="md"
				/>
				<div className="min-w-0 flex-1">
					<h3 className="text-ink truncate text-sm font-medium">
						{source.name}
					</h3>
					<p className="text-ink-faint text-xs">{source.adapter_id ?? source.data_type}</p>
				</div>
				<SourceStatusBadge status={source.status} />
			</div>

			<div className="text-ink-faint flex items-center justify-between text-xs">
				<span>
					{source.item_count.toLocaleString()} item
					{source.item_count !== 1 ? "s" : ""}
				</span>
				<span>
					{source.last_synced
						? formatRelative(source.last_synced)
						: "Never synced"}
				</span>
			</div>
		</button>
	);
}
