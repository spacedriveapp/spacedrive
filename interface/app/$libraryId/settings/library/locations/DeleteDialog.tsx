import { useLibraryMutation, usePlausibleEvent, useZodForm } from '@sd/client';
import { Dialog, UseDialogProps, useDialog } from '@sd/ui';

interface Props extends UseDialogProps {
	onSuccess: () => void;
	locationId: number;
}

export default (props: Props) => {
	const submitPlausibleEvent = usePlausibleEvent();

	const deleteLocation = useLibraryMutation('locations.delete', {
		onSuccess: () => {
			submitPlausibleEvent({ event: { type: 'locationDelete' } });
			props.onSuccess();
		}
	});

	const form = useZodForm();

	return (
		<Dialog
			form={form}
			onSubmit={form.handleSubmit(() => deleteLocation.mutateAsync(props.locationId))}
			dialog={useDialog(props)}
			title="Delete Location"
			description="Deleting a location will also remove all files associated with it from the Spacedrive database, the files themselves will not be deleted."
			ctaDanger
			ctaLabel="Delete"
		/>
	);
};
