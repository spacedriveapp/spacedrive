import { CaretRight, Database } from "@phosphor-icons/react";
import { useNavigate } from "react-router-dom";

interface SourcePathBarProps {
	sourceName: string;
	itemCount: number;
}

export function SourcePathBar({
	sourceName,
	itemCount,
}: SourcePathBarProps) {
	const navigate = useNavigate();

	return (
		<div
			className="border-app-line/50 bg-app-overlay/80 flex h-8 items-center gap-1.5 rounded-full border px-3 backdrop-blur-xl"
		>
			<Database size={14} className="text-ink-faint shrink-0" />

			<button
				onClick={() => navigate("/sources")}
				className="text-sidebar-inkDull hover:text-sidebar-ink whitespace-nowrap text-xs font-medium transition-colors"
			>
				Sources
			</button>

			<CaretRight size={12} className="text-ink-faint shrink-0 opacity-50" />

			<span className="text-sidebar-ink whitespace-nowrap text-xs font-medium">
				{sourceName}
			</span>

			<span className="text-ink-faint ml-1 whitespace-nowrap text-[11px]">
				{itemCount.toLocaleString()} items
			</span>
		</div>
	);
}
