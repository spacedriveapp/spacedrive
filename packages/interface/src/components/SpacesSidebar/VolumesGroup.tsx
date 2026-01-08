import { Plugs } from "@phosphor-icons/react";
import type { VolumeItem } from "@sd/ts-client";
import { getVolumeIcon, useNormalizedQuery } from "@sd/ts-client";
import { useNavigate } from "react-router-dom";
import { GroupHeader } from "./GroupHeader";
import { SpaceItem } from "./SpaceItem";

interface VolumesGroupProps {
  isCollapsed: boolean;
  onToggle: () => void;
  /** Filter to show tracked, untracked, or all volumes (default: "All") */
  filter?: "TrackedOnly" | "UntrackedOnly" | "All";
  sortableAttributes?: any;
  sortableListeners?: any;
}

export function VolumesGroup({
  isCollapsed,
  onToggle,
  filter = "All",
  sortableAttributes,
  sortableListeners,
}: VolumesGroupProps) {
  const navigate = useNavigate();

  const { data: volumesData } = useNormalizedQuery({
    wireMethod: "query:volumes.list",
    input: { filter },
    resourceType: "volume",
  });

  const volumes = volumesData?.volumes || [];

  // Helper to render volume status indicator
  const getVolumeIndicator = (volume: VolumeItem) => (
    <>
      {!volume.is_tracked && (
        <Plugs className="text-ink-faint" size={14} weight="bold" />
      )}
    </>
  );

  return (
    <div>
      <GroupHeader
        isCollapsed={isCollapsed}
        label="Volumes"
        onToggle={onToggle}
        sortableAttributes={sortableAttributes}
        sortableListeners={sortableListeners}
      />

      {/* Volumes List */}
      {!isCollapsed && (
        <div className="space-y-0.5">
          {volumes.length === 0 ? (
            <div className="px-2 py-1 text-ink-faint text-xs">No volumes</div>
          ) : (
            volumes.map((volume, index) => (
              <SpaceItem
                allowInsertion={false}
                customIcon={getVolumeIcon(volume)}
                isLastItem={index === volumes.length - 1}
                item={
                  {
                    id: volume.id,
                    item_type: {
                      Volume: {
                        volume_id: volume.id,
                        name: volume.display_name || volume.name,
                      },
                    },
                  } as any
                }
                key={volume.id}
                rightComponent={getVolumeIndicator(volume)}
                volumeData={{
                  device_slug: volume.device_slug,
                  mount_path: volume.mount_point || "/",
                }}
              />
            ))
          )}
        </div>
      )}
    </div>
  );
}
