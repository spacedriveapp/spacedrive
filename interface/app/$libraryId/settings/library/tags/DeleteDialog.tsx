import { useLibraryMutation, usePlausibleEvent } from '@sd/client';
import { Dialog, UseDialogProps, useDialog } from '@sd/ui';
import { useZodForm } from '@sd/ui/src/forms';

interface Props extends UseDialogProps {
	tagId: number;
	onSuccess: () => void;
}

export default (props: Props) => {
	const submitPlausibleEvent = usePlausibleEvent();

	const deleteTag = useLibraryMutation('tags.delete', {
		onSuccess: () => {
			submitPlausibleEvent({ event: { type: 'tagDelete' } });
			props.onSuccess();
		}
	});

	return (
		<Dialog
			form={useZodForm()}
			dialog={useDialog(props)}
			onSubmit={() => deleteTag.mutateAsync(props.tagId)}
			title="Delete Tag"
			description="Are you sure you want to delete this tag? This cannot be undone and tagged files will be unlinked."
			ctaDanger
			ctaLabel="Delete"
		/>
	);
};
