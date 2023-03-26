import { useLibraryMutation, usePlausibleEvent } from '@sd/client';
import { Dialog, UseDialogProps, useDialog } from '@sd/ui';
import { Input, useZodForm, z } from '@sd/ui/src/forms';
import ColorPicker from '~/components/ColorPicker';
import { usePlatform } from '~/util/Platform';

export default (props: UseDialogProps) => {
	const dialog = useDialog({ ...props, closeOnSubmit: false });
	const platform = usePlatform();
	const submitPlausibleEvent = usePlausibleEvent({ platformType: platform.platform });

	const form = useZodForm({
		schema: z.object({
			name: z.string().min(1, 'Name required'),
			color: z.string()
		}),
		defaultValues: {
			color: '#A717D9'
		}
	});

	const createTag = useLibraryMutation('tags.create', {
		onSuccess: () => {
			submitPlausibleEvent({ event: { type: 'tagCreate' } });
			dialog.close();
		},
		onError: (e) => {
			console.error('error', e);
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
			<Input
				placeholder="Name"
				icon={<ColorPicker control={form.control} name="color" />}
				outerClassName="mt-3"
				error={form.formState.errors.name?.message}
				{...form.register('name')}
			/>
		</Dialog>
	);
};
