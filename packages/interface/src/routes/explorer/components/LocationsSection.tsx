import { useDroppable } from "@dnd-kit/core";
import { Plus } from "@phosphor-icons/react";
import { Location } from "@sd/assets/icons";
import type { Location } from "@sd/ts-client";
import clsx from "clsx";
import { useEffect, useRef } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { useNormalizedQuery } from "../../../contexts/SpacedriveContext";
import { useEvent } from "../../../hooks/useEvent";
import { useAddLocationDialog } from "./AddLocationModal";
import { Section } from "./Section";
import { SidebarItem } from "./SidebarItem";

export function LocationsSection() {
  const navigate = useNavigate();
  const { locationId } = useParams();
  const previousLocationIdsRef = useRef<Set<string>>(new Set());

  const locationsQuery = useNormalizedQuery<null, Location>({
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

      if (
        resource_type === "location" &&
        typeof resource === "object" &&
        resource !== null
      ) {
        const newLocation = resource as Location;

        // Check if this is a new location (not in our previous set)
        if (!previousLocationIdsRef.current.has(newLocation.id)) {
          navigate(`/location/${newLocation.id}`);
        }
      }
    }
  });

  const handleLocationClick = (location: Location) => {
    navigate(`/location/${location.id}`);
  };

  const handleAddLocation = async () => {
    // Navigation now happens automatically via ResourceChanged event
    await useAddLocationDialog();
  };

  return (
    <Section title="Locations">
      {locationsQuery.isLoading && (
        <div className="px-2 py-1 text-sidebar-inkFaint text-xs">
          Loading...
        </div>
      )}

      {locationsQuery.error && (
        <div className="px-2 py-1 text-red-400 text-xs">
          Error: {(locationsQuery.error as Error).message}
        </div>
      )}

      {locations.length === 0 &&
        !locationsQuery.isLoading &&
        !locationsQuery.error && (
          <div className="px-2 py-1 text-sidebar-inkFaint text-xs">
            No locations yet
          </div>
        )}

      {locations.map((location) => (
        <LocationDropZone
          active={locationId === location.id}
          key={location.id}
          location={location}
          onClick={() => handleLocationClick(location)}
        />
      ))}

      <SidebarItem
        className="text-ink-faint hover:text-ink"
        icon={Plus}
        label="Add Location"
        onClick={handleAddLocation}
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
  location: Location;
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
    <div className="relative" ref={setNodeRef}>
      {isOver && (
        <div className="pointer-events-none absolute inset-0 z-10 rounded-lg ring-2 ring-accent ring-inset" />
      )}
      <SidebarItem
        active={active}
        className={clsx(isOver && "bg-accent/10")}
        icon={Location}
        label={location.name || "Unnamed"}
        onClick={onClick}
      />
    </div>
  );
}
