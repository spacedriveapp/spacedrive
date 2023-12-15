import {
	FilePath,
	Object,
	Target,
	ToastDefautlColor,
	useLibraryMutation,
	usePlausibleEvent,
	useZodForm
} from '@sd/client';
import { Dialog, InputField, useDialog, UseDialogProps, z } from '@sd/ui';
import { ColorPicker } from '~/components';

const schema = z.object({
	name: z.string().trim().min(1).max(24),
	color: z.string()
});

export type AssignTagItems = Array<
	{ type: 'Object'; item: Object } | { type: 'Path'; item: FilePath }
>;

export function useAssignItemsToTag() {
	const submitPlausibleEvent = usePlausibleEvent();

	const mutation = useLibraryMutation(['tags.assign'], {
		onSuccess: () => {
			submitPlausibleEvent({ event: { type: 'tagAssign' } });
		}
	});

	return (tagId: number, items: AssignTagItems, unassign: boolean = false) => {
		const targets = items.map<Target>((item) => {
			if (item.type === 'Object') {
				return { Object: item.item.id };
			} else {
				return { FilePath: item.item.id };
			}
		});

		return mutation.mutateAsync({
			tag_id: tagId,
			targets,
			unassign
		});
	};
}

export default (
	props: UseDialogProps & {
		items?: AssignTagItems;
	}
) => {
	const submitPlausibleEvent = usePlausibleEvent();

	const form = useZodForm({
		schema: schema,
		defaultValues: { color: ToastDefautlColor }
	});

	const createTag = useLibraryMutation('tags.create');

	const assignItemsToTag = useAssignItemsToTag();

	const onSubmit = form.handleSubmit(async (data) => {
		try {
			const tag = await createTag.mutateAsync(data);

			submitPlausibleEvent({ event: { type: 'tagCreate' } });

			if (props.items !== undefined) await assignItemsToTag(tag.id, props.items, false);
		} catch (e) {
			console.error('error', e);
		}
	});

	return (
		<Dialog
			invertButtonFocus
			form={form}
			onSubmit={onSubmit}
			dialog={useDialog(props)}
			title="Create New Tag"
			description="Choose a name and color."
			ctaLabel="Create"
		>
			<div className="relative mt-3 ">
				<InputField
					{...form.register('name', { required: true })}
					placeholder="Name"
					maxLength={24}
					icon={<ColorPicker control={form.control} name="color" />}
				/>
			</div>
		</Dialog>
	);
};
