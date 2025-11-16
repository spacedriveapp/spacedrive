/**
 * Overview Screen - The homepage of Spacedrive
 *
 * Now using real data from the backend!
 */

import { HeroStats } from "./HeroStats";
import { StorageOverview } from "./StorageOverview";
import { ProjectCards } from "./ProjectCards";
import { DevicesPanel } from "./DevicesPanel";
import { ContentBreakdown } from "./ContentBreakdown";
import { OverviewTopBar } from "./OverviewTopBar";
import { useNormalizedCache } from "../../context";
import type { LibraryInfoOutput } from "@sd/ts-client/generated/types";

export function Overview() {
	// Fetch library info with statistics using normalizedCache
	// This returns cached stats immediately and updates via ResourceChanged events
	const {
		data: libraryInfo,
		isLoading,
		error,
	} = useNormalizedCache<null, LibraryInfoOutput>({
		wireMethod: "query:libraries.info",
		input: null,
		resourceType: "library",
	});

	if (isLoading || !libraryInfo) {
		return (
			<>
				<OverviewTopBar libraryName="Loading..." />
				<div className="flex flex-col h-full overflow-hidden pt-[52px]">
					<div className="flex-1 overflow-auto p-6 space-y-6">
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

			<div className="flex flex-col h-full overflow-hidden pt-[52px]">
				{/* Main content - scrollable */}
				<div className="flex-1 overflow-auto p-6 space-y-6">
					{/* Hero Stats */}
					<HeroStats
						totalStorage={stats.total_capacity}
						usedStorage={
							stats.total_capacity - stats.available_capacity
						}
						totalFiles={Number(stats.total_files)}
						locationCount={stats.location_count}
						tagCount={stats.tag_count}
						deviceCount={stats.device_count}
						uniqueContentCount={Number(stats.unique_content_count)}
					/>
					{/* <ContentBreakdown totalFiles={Number(stats.total_files)} /> */}
				</div>
			</div>
		</>
	);
}
