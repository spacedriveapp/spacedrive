import { useState } from 'react';
import { Input, Label, dialogManager, useDialog, Dialog } from '@spacedrive/primitives';
import { useLibraryMutation } from '@sd/ts-client';
import { useForm } from 'react-hook-form';
import type { GroupType } from '@sd/ts-client';

interface FormData {
	groupName: string;
}

export function useAddGroupDialog(spaceId: string) {
	return dialogManager.create((props) => <AddGroupDialog {...props} spaceId={spaceId} />);
}

function AddGroupDialog(props: { id: number; spaceId: string }) {
	const dialog = useDialog(props);
	const [groupType, setGroupType] = useState<GroupType>('Custom');

	const form = useForm<FormData>({
		defaultValues: { groupName: '' },
	});

	const addGroup = useLibraryMutation('spaces.add_group');

	const onSubmit = form.handleSubmit(async (data) => {
		await addGroup.mutateAsync({
			space_id: props.spaceId,
			name: data.groupName || getDefaultName(groupType),
			group_type: groupType,
		});
		form.reset();
		setGroupType('Custom');
		dialog.state.open = false;
	});

	return (
		<Dialog form={form} dialog={dialog} title="Add Group" onSubmit={onSubmit} ctaLabel="Create">
			<div className="space-y-4">
				<div>
					<Label>Group Type</Label>
					<select
						value={typeof groupType === 'string' ? groupType : 'Custom'}
						onChange={(e) => setGroupType(e.target.value as GroupType)}
						className="w-full rounded-lg border border-app-line bg-app-input px-3 py-2 text-sm text-ink"
					>
						<option value="Devices">All Devices</option>
						<option value="Locations">All Locations</option>
						<option value="Tags">Tags</option>
						{/* Cloud option is hidden until CloudGroup renderer is implemented — see core/src/domain/space.rs (GroupType::Cloud variant) and .investigations/cloud-drives/06-mvp-onedrive-vertical-slice.md#pr-7 */}
						<option value="Custom">Custom</option>
					</select>
				</div>

				{groupType === 'Custom' && (
					<div>
						<Label>Group Name</Label>
						<Input
							{...form.register('groupName')}
							placeholder="Enter group name"
						/>
					</div>
				)}
			</div>
		</Dialog>
	);
}

function getDefaultName(groupType: GroupType): string {
	if (groupType === 'Devices') return 'Devices';
	if (groupType === 'Locations') return 'Locations';
	if (groupType === 'Tags') return 'Tags';
	if (groupType === 'Cloud') return 'Cloud';
	if (groupType === 'Custom') return 'Custom Group';
	if (typeof groupType === 'object' && 'Device' in groupType) return 'Device';
	return 'Group';
}

