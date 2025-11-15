import { Plus } from '@phosphor-icons/react';
import { useAddGroupDialog } from './AddGroupModal';

interface AddGroupButtonProps {
	spaceId: string;
}

export function AddGroupButton({ spaceId }: AddGroupButtonProps) {
	const addGroupDialog = useAddGroupDialog;

	return (
		<button
			onClick={() => addGroupDialog(spaceId)}
			className="flex w-full items-center gap-2 rounded-lg border border-dashed border-sidebar-line/70 px-2 py-1.5 text-sm font-medium text-ink-faint hover:border-sidebar-line hover:bg-sidebar-selected/5 hover:text-sidebar-ink"
		>
			<Plus size={16} weight="bold" />
			<span>Add Group</span>
		</button>
	);
}
