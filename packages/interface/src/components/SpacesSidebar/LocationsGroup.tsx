import { useNavigate } from "react-router-dom";
import { useNormalizedQuery } from "@sd/ts-client";
import { SpaceItem } from "./SpaceItem";
import { GroupHeader } from "./GroupHeader";

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
      <GroupHeader label="Locations" isCollapsed={isCollapsed} onToggle={onToggle} />

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
