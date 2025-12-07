import { CaretRight, Folder } from "@phosphor-icons/react";
import clsx from "clsx";
import { useNavigate } from "react-router-dom";
import { useNormalizedQuery } from "@sd/ts-client";
import { SpaceItem } from "./SpaceItem";

interface LocationsGroupProps {
  isCollapsed: boolean;
  onToggle: () => void;
}

export function LocationsGroup({ isCollapsed, onToggle }: LocationsGroupProps) {
  const navigate = useNavigate();

  const { data: locationsData } = useNormalizedQuery({
    wireMethod: "query:locations.list",
    input: null, // Unit struct serializes as null, not {}
    resourceType: "location",
  });

  const locations = locationsData?.locations ?? [];

  return (
    <div>
      {/* Header */}
      <button
        onClick={onToggle}
        className="mb-1 flex w-full cursor-default items-center gap-2 px-1 text-xs font-semibold uppercase tracking-wider text-sidebar-ink-faint hover:text-sidebar-ink"
      >
        <CaretRight
          className={clsx("transition-transform", !isCollapsed && "rotate-90")}
          size={10}
          weight="bold"
        />
        <span>Locations</span>
      </button>

      {/* Items */}
      {!isCollapsed && (
        <div className="space-y-0.5">
          {locations.map((location, index) => (
            <SpaceItem
              key={location.id}
              item={location}
              allowInsertion={false}
              isLastItem={index === locations.length - 1}
            />
          ))}
        </div>
      )}
    </div>
  );
}
