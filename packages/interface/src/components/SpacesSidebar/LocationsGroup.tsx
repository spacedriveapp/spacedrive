import { useNormalizedQuery } from "@sd/ts-client";
import { useNavigate } from "react-router-dom";
import { GroupHeader } from "./GroupHeader";
import { SpaceItem } from "./SpaceItem";

interface LocationsGroupProps {
  isCollapsed: boolean;
  onToggle: () => void;
  sortableAttributes?: any;
  sortableListeners?: any;
}

export function LocationsGroup({
  isCollapsed,
  onToggle,
  sortableAttributes,
  sortableListeners,
}: LocationsGroupProps) {
  const navigate = useNavigate();

  const { data: locationsData } = useNormalizedQuery({
    wireMethod: "query:locations.list",
    input: null, // Unit struct serializes as null, not {}
    resourceType: "location",
  });

  const locations = locationsData?.locations ?? [];

  return (
    <div>
      <GroupHeader
        isCollapsed={isCollapsed}
        label="Locations"
        onToggle={onToggle}
        sortableAttributes={sortableAttributes}
        sortableListeners={sortableListeners}
      />

      {/* Items */}
      {!isCollapsed && (
        <div className="space-y-0.5">
          {locations.map((location, index) => (
            <SpaceItem
              allowInsertion={false}
              isLastItem={index === locations.length - 1}
              item={location}
              key={location.id}
            />
          ))}
        </div>
      )}
    </div>
  );
}
