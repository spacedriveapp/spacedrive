import { Object, useLibraryMutation, usePlausibleEvent } from '@sd/client';
import { Dialog, InputField, UseDialogProps, useDialog, useZodForm, z } from '@sd/ui';
import { ColorPicker } from '~/components';

const schema = z.object({
	name: z.string().trim().min(1).max(24),
	color: z.string()
});

export default (props: UseDialogProps & { objects?: Object[] }) => {
	const submitPlausibleEvent = usePlausibleEvent();

	const form = useZodForm({
		schema: schema,
		defaultValues: { color: '#A717D9' }
	});

	const createTag = useLibraryMutation('tags.create');
	const assignTag = useLibraryMutation('tags.assign');

	const onSubmit = form.handleSubmit(async (data) => {
		try {
			const tag = await createTag.mutateAsync(data);

			submitPlausibleEvent({ event: { type: 'tagCreate' } });

			if (props.objects !== undefined) {
				await assignTag.mutateAsync({
					tag_id: tag.id,
					object_ids: props.objects.map((o) => o.id),
					unassign: false
				});
			}
		} catch (e) {
			console.error('error', e);
		}
	});

	return (
		<Dialog
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
