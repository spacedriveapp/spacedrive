import { Plus } from "@phosphor-icons/react";
import { useAddGroupDialog } from "./AddGroupModal";

interface AddGroupButtonProps {
  spaceId: string;
}

export function AddGroupButton({ spaceId }: AddGroupButtonProps) {
  const addGroupDialog = useAddGroupDialog;

  return (
    <button
      className="flex w-full items-center gap-2 rounded-lg border border-sidebar-line/70 border-dashed px-2 py-1.5 font-medium text-ink-faint text-sm hover:border-sidebar-line hover:bg-sidebar-selected/5 hover:text-sidebar-ink"
      onClick={() => addGroupDialog(spaceId)}
    >
      <Plus size={16} weight="bold" />
      <span>Add Group</span>
    </button>
  );
}
