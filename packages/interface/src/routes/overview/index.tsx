/**
 * Overview Screen - The homepage of Spacedrive
 *
 * Now using real data from the backend!
 */

import type {
  Library,
  LocationsListOutput,
  LocationsListQueryInput,
} from "@sd/ts-client";
import { useMemo, useState } from "react";
import { Inspector } from "../../components/Inspector/Inspector";
import { useNormalizedQuery } from "../../contexts/SpacedriveContext";
import { DevicePanel } from "./DevicePanel";
import { HeroStats } from "./HeroStats";
import { OverviewTopBar } from "./OverviewTopBar";

export function Overview() {
  const [selectedLocationId, setSelectedLocationId] = useState<string | null>(
    null
  );

  // Fetch library info with statistics using normalizedCache
  // This returns cached stats immediately and updates via ResourceChanged events
  const {
    data: libraryInfo,
    isLoading,
    error,
  } = useNormalizedQuery<null, Library>({
    wireMethod: "query:libraries.info",
    input: null,
    resourceType: "library",
  });

  // Fetch locations list to get the selected location reactively
  const { data: locationsData } = useNormalizedQuery<
    LocationsListQueryInput,
    LocationsListOutput
  >({
    wireMethod: "query:locations.list",
    input: null,
    resourceType: "location",
  });

  // Find the selected location from the list reactively
  const selectedLocation = useMemo(() => {
    if (!(selectedLocationId && locationsData?.locations)) return null;
    return (
      locationsData.locations.find((loc) => loc.id === selectedLocationId) ||
      null
    );
  }, [selectedLocationId, locationsData]);

  if (isLoading || !libraryInfo) {
    return (
      <>
        <OverviewTopBar libraryName="Loading..." />
        <div className="flex h-full flex-col overflow-hidden">
          <div className="flex-1 space-y-4 overflow-auto p-6">
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

      <div className="flex h-full flex-col overflow-hidden">
        <div className="flex flex-1 gap-2 overflow-hidden">
          {/* Main content - scrollable */}
          <div className="flex-1 space-y-4 overflow-auto p-3">
            {/* Hero Stats */}
            <HeroStats
              deviceCount={stats.device_count}
              locationCount={stats.location_count}
              tagCount={stats.tag_count}
              totalFiles={Number(stats.total_files)}
              totalStorage={stats.total_capacity}
              uniqueContentCount={Number(stats.unique_content_count)}
              usedStorage={stats.total_capacity - stats.available_capacity}
            />

            {/* Device Panel */}
            <DevicePanel
              onLocationSelect={(location) =>
                setSelectedLocationId(location?.id || null)
              }
            />

            {/* <ContentBreakdown totalFiles={Number(stats.total_files)} /> */}
          </div>

          {/* Inspector Sidebar */}
          {selectedLocation && (
            <div className="w-[300px] flex-shrink-0 py-2 pr-2">
              <Inspector
                currentLocation={selectedLocation as any}
                showPopOutButton={false}
              />
            </div>
          )}
        </div>
      </div>
    </>
  );
}
