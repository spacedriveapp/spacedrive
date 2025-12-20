import { CaretRight, DotsSixVertical, PencilSimple, Trash } from "@phosphor-icons/react";
import clsx from "clsx";
import { useState } from "react";
import { useContextMenu } from "../../hooks/useContextMenu";
import { useLibraryMutation } from "@sd/ts-client";
import type { SpaceGroup } from "@sd/ts-client";

interface GroupHeaderProps {
  label: string;
  isCollapsed: boolean;
  onToggle: () => void;
  rightComponent?: React.ReactNode;
  sortableAttributes?: any;
  sortableListeners?: any;
  group?: SpaceGroup;
  allowCustomization?: boolean;
}

export function GroupHeader({
  label,
  isCollapsed,
  onToggle,
  rightComponent,
  sortableAttributes,
  sortableListeners,
  group,
  allowCustomization = false,
}: GroupHeaderProps) {
  const hasSortable = sortableAttributes && sortableListeners;
  const [isRenaming, setIsRenaming] = useState(false);
  const [newName, setNewName] = useState(label);
  
  const updateGroup = useLibraryMutation("spaces.update_group");
  const deleteGroup = useLibraryMutation("spaces.delete_group");

  const handleRename = async () => {
    if (!group || !newName.trim() || newName === label) {
      setIsRenaming(false);
      setNewName(label);
      return;
    }

    try {
      await updateGroup.mutateAsync({
        group_id: group.id,
        name: newName.trim(),
      });
      setIsRenaming(false);
    } catch (error) {
      console.error("Failed to rename group:", error);
      setNewName(label);
      setIsRenaming(false);
    }
  };

  const handleDelete = async () => {
    if (!group) return;
    
    try {
      await deleteGroup.mutateAsync({ group_id: group.id });
    } catch (error) {
      console.error("Failed to delete group:", error);
    }
  };

  const contextMenu = useContextMenu({
    items: [
      {
        icon: PencilSimple,
        label: "Rename Group",
        onClick: () => {
          setNewName(label);
          setIsRenaming(true);
        },
        condition: () => allowCustomization,
      },
      { type: "separator" },
      {
        icon: Trash,
        label: "Delete Group",
        onClick: handleDelete,
        variant: "danger" as const,
        condition: () => allowCustomization,
      },
    ],
  });

  const handleContextMenu = async (e: React.MouseEvent) => {
    if (!allowCustomization) return;
    e.preventDefault();
    e.stopPropagation();
    await contextMenu.show(e);
  };

  return (
    <div className="mb-1 flex w-full items-center gap-1 px-1">
      {/* Drag Handle - Only show if sortable */}
      {hasSortable && (
        <div
          {...sortableAttributes}
          {...sortableListeners}
          className="cursor-grab active:cursor-grabbing py-2 px-0.5 -ml-1 text-sidebar-inkFaint hover:text-sidebar-ink transition-colors"
        >
          <DotsSixVertical size={12} weight="bold" />
        </div>
      )}

      {/* Collapsible Button or Rename Input */}
      {isRenaming ? (
        <input
          type="text"
          value={newName}
          onChange={(e) => setNewName(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              handleRename();
            } else if (e.key === "Escape") {
              setIsRenaming(false);
              setNewName(label);
            }
          }}
          onBlur={handleRename}
          autoFocus
          className="flex-1 px-2 py-1 text-tiny font-semibold tracking-wider rounded-md bg-sidebar-box border border-sidebar-line text-sidebar-ink placeholder:text-sidebar-ink-faint outline-none focus:border-accent"
        />
      ) : (
        <button
          onClick={onToggle}
          onContextMenu={handleContextMenu}
          className="flex-1 flex items-center gap-2 py-2 text-tiny font-semibold tracking-wider opacity-60 text-sidebar-ink-faint hover:text-sidebar-ink transition-colors min-h-[32px]"
        >
          <CaretRight
            className={clsx("transition-transform", !isCollapsed && "rotate-90")}
            size={10}
            weight="bold"
          />
          <span>{label}</span>
          {rightComponent}
        </button>
      )}
    </div>
  );
}
