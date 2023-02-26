import { useLibraryMutation } from '@sd/client';
import { Dialog, UseDialogProps, useDialog } from '@sd/ui';
import { useZodForm } from '@sd/ui/src/forms';

interface Props extends UseDialogProps {
	tagId: number;
	onSuccess: () => void;
}

export default (props: Props) => {
	const dialog = useDialog(props);

	const form = useZodForm();

	const deleteTag = useLibraryMutation('tags.delete', {
		onSuccess: props.onSuccess
	});

	return (
		<Dialog
			{...{ form, dialog }}
			onSubmit={form.handleSubmit(() => deleteTag.mutateAsync(props.tagId))}
			title="Delete Tag"
			description="Are you sure you want to delete this tag? This cannot be undone and tagged files will be unlinked."
			ctaDanger
			ctaLabel="Delete"
		/>
	);
};
