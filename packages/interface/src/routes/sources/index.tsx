import { Plus, ArrowLeft } from "@phosphor-icons/react";
import { useNavigate } from "react-router-dom";
import { useLibraryQuery } from "../../contexts/SpacedriveContext";
import { useTabManager } from "../../components/TabManager/useTabManager";
import { SourceCard } from "../../components/Sources/SourceCard";
import { TopBarPortal, TopBarItem } from "../../TopBar";
import { CircleButton } from "@spaceui/primitives";
import { SearchBar } from "@spaceui/primitives";

export function SourcesHome() {
	const navigate = useNavigate();
	const { createTab } = useTabManager();
	const { data: sources, isLoading, error } = useLibraryQuery({
		type: "sources.list",
		input: { data_type: null },
	});

	return (
		<>
			<TopBarPortal
				left={
					<>
						<TopBarItem id="back" label="Back" priority="high">
							<CircleButton
								icon={ArrowLeft}
								onClick={() => navigate(-1)}
							/>
						</TopBarItem>
						<TopBarItem id="title" label="Title" priority="high">
							<h1 className="text-ink text-xl font-bold">
								Sources
							</h1>
						</TopBarItem>
					</>
				}
				right={
					<>
						<TopBarItem id="search" label="Search" priority="high">
								<SearchBar
								placeholder="Search sources..."
								value=""
								onChange={() => {}}
								onClear={() => {}}
								className="w-64"
							/>
						</TopBarItem>
						<TopBarItem id="add-source" label="Add Source" priority="high">
							<CircleButton
								icon={Plus}
								onClick={() => createTab("Adapters", "/sources/adapters")}
								title="Add Source"
							/>
						</TopBarItem>
					</>
				}
			/>
			<div className="p-6">

			{isLoading && (
				<div className="flex items-center justify-center py-20">
					<div className="text-ink-faint text-sm">Loading...</div>
				</div>
			)}

			{error && (
				<div className="border-red-400/20 rounded-lg border p-4">
					<p className="text-sm text-red-400">
						Failed to load sources: {String(error)}
					</p>
				</div>
			)}

			{sources && sources.length === 0 && (
				<div className="flex flex-col items-center justify-center py-20">
					<p className="text-ink-dull text-sm">No sources yet</p>
					<p className="text-ink-faint mt-1 text-xs">
						Add a data source to get started
					</p>
					<button
						onClick={() => createTab("Adapters", "/sources/adapters")}
						className="bg-accent hover:bg-accent-deep mt-4 rounded-lg px-3.5 py-1.5 text-sm font-medium text-white transition-colors"
					>
						Add Source
					</button>
				</div>
			)}

			{sources && sources.length > 0 && (
				<div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
					{sources.map((source) => (
						<SourceCard key={source.id} source={source} />
					))}
				</div>
			)}
			</div>
		</>
	);
}
