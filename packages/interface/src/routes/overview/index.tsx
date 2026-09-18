/**
 * Overview Screen - The homepage of Spacedrive
 *
 * Now using real data from the backend!
 */

import { HeroStats } from "./HeroStats";
import { DevicePanel } from "./DevicePanel";
import { OverviewTopBar } from "./OverviewTopBar";
import { useNormalizedQuery } from "../../contexts/SpacedriveContext";
import type { Library } from "@sd/ts-client";

export function Overview() {
	// Fetch library info with statistics using normalizedCache
	// This returns cached stats immediately and updates via ResourceChanged events
	const {
		data: libraryInfo,
		isLoading,
	} = useNormalizedQuery<null, Library>({
		query: "libraries.info",
		input: null,
		resourceType: "library",
	});

	if (isLoading || !libraryInfo) {
		return (
			<>
				<OverviewTopBar libraryName="Loading..." />
				<div className="flex flex-col h-full overflow-hidden">
					<div className="flex-1 overflow-auto p-6 space-y-4">
						<div className="text-center text-ink-dull">
							Loading library statistics...
						</div>
					</div>
				</div>
			</>
		);
	}

	const stats = libraryInfo.statistics;

	return (
		<>
			<OverviewTopBar libraryName={libraryInfo.name} />

			<div className="flex flex-col h-full overflow-hidden">
				<div className="flex-1 flex gap-2 overflow-hidden">
					{/* Main content - scrollable */}
					<div className="flex-1 overflow-auto p-3 space-y-4">
						{/* Hero Stats */}
						<HeroStats
							totalStorage={stats.total_capacity ?? 0}
							usedStorage={
								(stats.total_capacity ?? 0) - (stats.available_capacity ?? 0)
							}
							totalFiles={Number(stats.total_files)}
							sourceCount={stats.source_count ?? 0}
							tagCount={stats.tag_count}
							deviceCount={stats.device_count ?? 0}
							uniqueContentCount={Number(
								stats.unique_content_count ?? 0,
							)}
							databaseSize={Number(stats.database_size)}
							sidecarCount={Number(stats.sidecar_count ?? 0)}
							sidecarSize={Number(stats.sidecar_size ?? 0)}
						/>

						{/* Device Panel */}
						<DevicePanel />

						{/* <ContentBreakdown totalFiles={Number(stats.total_files)} /> */}
					</div>

				</div>
			</div>
		</>
	);
}