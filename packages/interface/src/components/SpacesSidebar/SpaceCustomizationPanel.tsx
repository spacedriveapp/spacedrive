import { useDraggable } from "@dnd-kit/core";
import { Plus, X } from "@phosphor-icons/react";
import type {
  GroupType,
  ItemType,
  SpaceItem as SpaceItemType,
} from "@sd/ts-client";
import { Input } from "@sd/ui";
import clsx from "clsx";
import { AnimatePresence, motion } from "framer-motion";
import { useState } from "react";
import { createPortal } from "react-dom";
import { useLibraryMutation } from "../../contexts/SpacedriveContext";
import { SpaceItem } from "./SpaceItem";

interface PaletteItem {
  type: ItemType;
  label: string;
}

const PALETTE_ITEMS: PaletteItem[] = [
  {
    type: "Overview",
    label: "Overview",
  },
  {
    type: "Recents",
    label: "Recents",
  },
  {
    type: "Favorites",
    label: "Favorites",
  },
  {
    type: "FileKinds",
    label: "File Kinds",
  },
];

function DraggablePaletteItem({ item }: { item: PaletteItem }) {
  const { attributes, listeners, setNodeRef, isDragging } = useDraggable({
    id: `palette-${item.label}`,
    data: {
      type: "palette-item",
      itemType: item.type,
    },
  });

  // Create a mock SpaceItem for rendering
  const mockSpaceItem: SpaceItemType = {
    id: `palette-${item.label}`,
    space_id: "",
    group_id: null,
    item_type: item.type,
    order: 0,
    created_at: new Date().toISOString(),
  };

  return (
    <div
      ref={setNodeRef}
      {...attributes}
      {...listeners}
      className={clsx(
        "cursor-move transition-opacity",
        isDragging && "opacity-50"
      )}
    >
      <SpaceItem
        allowInsertion={false}
        item={mockSpaceItem}
        onClick={(e) => {
          e.preventDefault();
          e.stopPropagation();
        }}
      />
    </div>
  );
}

interface SpaceCustomizationPanelProps {
  isOpen: boolean;
  onClose: () => void;
  spaceId: string | null;
}

function getDefaultGroupName(groupType: GroupType): string {
  if (groupType === "Devices") return "Devices";
  if (groupType === "Locations") return "Locations";
  if (groupType === "Tags") return "Tags";
  if (groupType === "Cloud") return "Cloud";
  if (groupType === "Custom") return "Custom Group";
  if (typeof groupType === "object" && "Device" in groupType) return "Device";
  return "Group";
}

export function SpaceCustomizationPanel({
  isOpen,
  onClose,
  spaceId,
}: SpaceCustomizationPanelProps) {
  const [groupType, setGroupType] = useState<GroupType>("Custom");
  const [groupName, setGroupName] = useState("");
  const [isAddingGroup, setIsAddingGroup] = useState(false);
  const addGroup = useLibraryMutation("spaces.add_group");

  if (!spaceId) return null;

  const handleAddGroup = async () => {
    if (!spaceId) return;

    try {
      const result = await addGroup.mutateAsync({
        space_id: spaceId,
        name: groupName.trim() || getDefaultGroupName(groupType),
        group_type: groupType,
      });

      // Reset form
      setGroupName("");
      setGroupType("Custom");
      setIsAddingGroup(false);

      // Scroll to the newly created group in the sidebar after a brief delay
      // (allows time for the group to be added to the DOM)
      setTimeout(() => {
        const groupElement = document.querySelector(
          `[data-group-id="${result.group.id}"]`
        );
        if (groupElement) {
          groupElement.scrollIntoView({
            behavior: "smooth",
            block: "nearest",
          });
          // Add a temporary highlight effect
          groupElement.classList.add("ring-2", "ring-accent/50");
          setTimeout(() => {
            groupElement.classList.remove("ring-2", "ring-accent/50");
          }, 2000);
        }
      }, 100);
    } catch (err) {
      console.error("Failed to add group:", err);
    }
  };

  const content = (
    <AnimatePresence>
      {isOpen && (
        <>
          {/* Backdrop */}
          <motion.div
            animate={{ opacity: 1 }}
            className="fixed inset-0 z-[65] bg-black/20"
            exit={{ opacity: 0 }}
            initial={{ opacity: 0 }}
            onClick={onClose}
            transition={{ duration: 0.2 }}
          />

          {/* Panel */}
          <motion.div
            animate={{ x: 0, opacity: 1 }}
            className="fixed top-2 bottom-2 left-[228px] z-[70] w-[220px]"
            exit={{ x: -20, opacity: 0 }}
            initial={{ x: -20, opacity: 0 }}
            transition={{ duration: 0.3, ease: [0.25, 1, 0.5, 1] }}
          >
            <div className="flex h-full flex-col rounded-2xl bg-sidebar p-2.5">
              {/* Header */}
              <div className="mb-2 flex items-center justify-between px-2 py-2">
                <div>
                  <h2 className="font-semibold text-sidebar-ink text-sm">
                    Customize
                  </h2>
                  <p className="mt-0.5 text-sidebar-inkDull text-xs">
                    Drag to sidebar
                  </p>
                </div>
                <button
                  className="rounded-md p-1 transition-colors hover:bg-sidebar-selected/30"
                  onClick={onClose}
                >
                  <X className="text-sidebar-inkDull" size={14} />
                </button>
              </div>

              {/* Content */}
              <div className="flex-1 space-y-4 overflow-y-auto">
                {/* Quick Access Items */}
                <div className="space-y-0.5">
                  {PALETTE_ITEMS.map((item) => (
                    <DraggablePaletteItem item={item} key={item.label} />
                  ))}
                </div>

                {/* Add Group Section */}
                <div className="space-y-2 border-sidebar-line/50 border-t pt-2">
                  <div className="flex items-center justify-between px-2">
                    <span className="font-semibold text-sidebar-inkDull text-xs uppercase tracking-wider">
                      Groups
                    </span>
                  </div>

                  {isAddingGroup ? (
                    <div className="space-y-2 px-2">
                      <select
                        className="w-full rounded-md border border-sidebar-line bg-sidebar-box px-2 py-1.5 text-sidebar-ink text-xs focus:outline-none focus:ring-1 focus:ring-accent"
                        onChange={(e) =>
                          setGroupType(e.target.value as GroupType)
                        }
                        value={
                          typeof groupType === "string" ? groupType : "Custom"
                        }
                      >
                        <option value="Devices">All Devices</option>
                        <option value="Locations">All Locations</option>
                        <option value="Tags">Tags</option>
                        <option value="Cloud">Cloud Storage</option>
                        <option value="Custom">Custom</option>
                      </select>

                      {groupType === "Custom" && (
                        <Input
                          autoFocus
                          className="text-xs"
                          onChange={(e) => setGroupName(e.target.value)}
                          onKeyDown={(e) => {
                            if (e.key === "Enter") {
                              handleAddGroup();
                            } else if (e.key === "Escape") {
                              setIsAddingGroup(false);
                              setGroupName("");
                            }
                          }}
                          placeholder="Group name"
                          value={groupName}
                        />
                      )}

                      <div className="flex gap-2">
                        <button
                          className="flex-1 rounded-md bg-accent px-2 py-1 font-medium text-white text-xs transition-colors hover:bg-accent/90"
                          onClick={handleAddGroup}
                        >
                          Add
                        </button>
                        <button
                          className="rounded-md px-2 py-1 font-medium text-sidebar-inkDull text-xs transition-colors hover:bg-sidebar-selected/30"
                          onClick={() => {
                            setIsAddingGroup(false);
                            setGroupName("");
                            setGroupType("Custom");
                          }}
                        >
                          Cancel
                        </button>
                      </div>
                    </div>
                  ) : (
                    <button
                      className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 font-medium text-sidebar-inkDull text-sm transition-colors hover:bg-sidebar-selected/30 hover:text-sidebar-ink"
                      onClick={() => setIsAddingGroup(true)}
                    >
                      <Plus size={16} weight="bold" />
                      <span>Add Group</span>
                    </button>
                  )}
                </div>
              </div>

              {/* Footer */}
              <div className="mt-2 border-sidebar-line/50 border-t px-2 py-2">
                <p className="text-center text-sidebar-inkFaint text-xs">
                  Drag items to your space
                </p>
              </div>
            </div>
          </motion.div>
        </>
      )}
    </AnimatePresence>
  );

  return createPortal(content, document.body);
}
