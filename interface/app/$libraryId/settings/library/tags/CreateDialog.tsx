import { useLibraryMutation, usePlausibleEvent } from '@sd/client';
import { Dialog, UseDialogProps, useDialog } from '@sd/ui';
import { Input, useZodForm, z } from '@sd/ui/src/forms';
import ColorPicker from '~/components/ColorPicker';

export default (props: UseDialogProps & { assignToObject?: number }) => {
	const dialog = useDialog(props);
	const submitPlausibleEvent = usePlausibleEvent();

	const form = useZodForm({
		schema: z.object({
			name: z.string(),
			color: z.string()
		}),
		defaultValues: {
			color: '#A717D9'
		}
	});

	const createTag = useLibraryMutation('tags.create', {
		onSuccess: (tag) => {
			submitPlausibleEvent({ event: { type: 'tagCreate' } });
			if (props.assignToObject !== undefined) {
				assignTag.mutate({
					tag_id: tag.id,
					object_id: props.assignToObject,
					unassign: false
				});
			}
		},
		onError: (e) => {
			console.error('error', e);
		}
	});

	const assignTag = useLibraryMutation('tags.assign', {
		onSuccess: () => {
			submitPlausibleEvent({ event: { type: 'tagAssign' } });
		}
	});

	return (
		<Dialog
			{...{ dialog, form }}
			onSubmit={form.handleSubmit((data) => createTag.mutateAsync(data))}
			title="Create New Tag"
			description="Choose a name and color."
			ctaLabel="Create"
		>
			<div className="relative mt-3 ">
				<Input
					{...form.register('name', { required: true })}
					placeholder="Name"
					icon={<ColorPicker control={form.control} name="color" />}
				/>
			</div>
		</Dialog>
	);
};
