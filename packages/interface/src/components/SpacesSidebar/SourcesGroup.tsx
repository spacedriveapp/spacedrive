import { useNavigate, useLocation } from "react-router-dom";
import { Database } from "@phosphor-icons/react";
import { useLibraryQuery } from "../../contexts/SpacedriveContext";
import { useAdapterIcons } from "../../hooks/useAdapterIcons";
import { GroupHeader } from "./GroupHeader";

interface SourcesGroupProps {
	isCollapsed: boolean;
	onToggle: () => void;
	sortableAttributes?: any;
	sortableListeners?: any;
}

export function SourcesGroup({
	isCollapsed,
	onToggle,
	sortableAttributes,
	sortableListeners,
}: SourcesGroupProps) {
	const navigate = useNavigate();
	const location = useLocation();
	const { getIcon } = useAdapterIcons();

	const { data: sources } = useLibraryQuery({
		type: "sources.list",
		input: { data_type: null },
	});

	const sourcesList = sources ?? [];

	return (
		<div>
			<GroupHeader
				label="Sources"
				isCollapsed={isCollapsed}
				onToggle={onToggle}
				sortableAttributes={sortableAttributes}
				sortableListeners={sortableListeners}
			/>

			{!isCollapsed && (
				<div className="space-y-0.5">
					{sourcesList.map((source) => {
						const isActive = location.pathname === `/sources/${source.id}`;
						return (
							<button
								key={source.id}
								onClick={() => navigate(`/sources/${source.id}`)}
								className={`flex w-full items-center gap-2 rounded-md px-2 py-1 text-left text-sm font-medium ${
									isActive
										? "bg-sidebar-selected text-sidebar-ink"
										: "text-sidebar-inkDull hover:text-sidebar-ink"
								}`}
							>
								{getIcon(source.adapter_id) ? (
									<div
										className={`size-4 shrink-0 [&>svg]:h-full [&>svg]:w-full ${
											isActive
												? "opacity-100"
												: "opacity-60 grayscale"
										}`}
										dangerouslySetInnerHTML={{
											__html: getIcon(source.adapter_id)!,
										}}
									/>
								) : (
									<Database
										className="size-4 shrink-0"
										weight={isActive ? "fill" : "bold"}
									/>
								)}
								<span className="truncate">{source.name}</span>
							</button>
						);
					})}
				</div>
			)}
		</div>
	);
}
