import { useNavigate, useParams } from "react-router-dom";
import { useRef, useEffect } from "react";
import { Plus } from "@phosphor-icons/react";
import clsx from "clsx";
import type { LocationInfo } from "@sd/ts-client";
import { useNormalizedQuery } from "../../../context";
import { Section } from "./Section";
import { SidebarItem } from "./SidebarItem";
import { useAddLocationDialog } from "./AddLocationModal";
import { Location } from "@sd/assets/icons";
import { useEvent } from "../../../hooks/useEvent";
import { useDroppable } from "@dnd-kit/core";

export function LocationsSection() {
  const navigate = useNavigate();
  const { locationId } = useParams();
  const previousLocationIdsRef = useRef<Set<string>>(new Set());

  const locationsQuery = useNormalizedQuery<null, LocationInfo>({
    wireMethod: "query:locations.list",
    input: null,
    resourceType: "location",
  });

  const locations = locationsQuery.data?.locations || [];

  // Track location IDs to detect new locations
  useEffect(() => {
    previousLocationIdsRef.current = new Set(locations.map((loc) => loc.id));
  }, [locations]);

  // Listen for new location creation events and navigate to them
  useEvent("ResourceChanged", (event) => {
    if ("ResourceChanged" in event) {
      const { resource_type, resource } = event.ResourceChanged;

      if (resource_type === "location" && typeof resource === "object" && resource !== null) {
        const newLocation = resource as LocationInfo;

        // Check if this is a new location (not in our previous set)
        if (!previousLocationIdsRef.current.has(newLocation.id)) {
          navigate(`/location/${newLocation.id}`);
        }
      }
    }
  });

  const handleLocationClick = (location: LocationInfo) => {
    navigate(`/location/${location.id}`);
  };

  const handleAddLocation = async () => {
    // Navigation now happens automatically via ResourceChanged event
    await useAddLocationDialog();
  };

  return (
    <Section title="Locations">
      {locationsQuery.isLoading && (
        <div className="px-2 py-1 text-xs text-sidebar-inkFaint">
          Loading...
        </div>
      )}

      {locationsQuery.error && (
        <div className="px-2 py-1 text-xs text-red-400">
          Error: {(locationsQuery.error as Error).message}
        </div>
      )}

      {locations.length === 0 &&
        !locationsQuery.isLoading &&
        !locationsQuery.error && (
          <div className="px-2 py-1 text-xs text-sidebar-inkFaint">
            No locations yet
          </div>
        )}

      {locations.map((location) => (
        <LocationDropZone
          key={location.id}
          location={location}
          active={locationId === location.id}
          onClick={() => handleLocationClick(location)}
        />
      ))}

      <SidebarItem
        icon={Plus}
        label="Add Location"
        onClick={handleAddLocation}
        className="text-ink-faint hover:text-ink"
      />
    </Section>
  );
}

// Location item with drop zone support
function LocationDropZone({
  location,
  active,
  onClick,
}: {
  location: LocationInfo;
  active: boolean;
  onClick: () => void;
}) {
  const { setNodeRef, isOver } = useDroppable({
    id: `location-drop-${location.id}`,
    data: {
      action: "move-into",
      targetType: "location",
      targetId: location.id,
      targetPath: location.sd_path, // Use the proper sd_path from the location
    },
  });

  return (
    <div ref={setNodeRef} className="relative">
      {isOver && (
        <div className="absolute inset-0 rounded-lg ring-2 ring-accent ring-inset pointer-events-none z-10" />
      )}
      <SidebarItem
        icon={Location}
        label={location.name || "Unnamed"}
        active={active}
        onClick={onClick}
        className={clsx(isOver && "bg-accent/10")}
      />
    </div>
  );
}
