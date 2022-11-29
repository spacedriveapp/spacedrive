import { useLibraryMutation } from '@sd/client';
import { useState } from 'react';
import Dialog from '~/components/layout/Dialog';

type Props = {
	locationId: number;
	// Fires when location is deleted
	onSubmit?: () => void;
	children: React.ReactNode;
};

const DeleteLocationDialog = ({ children, onSubmit, locationId }: Props) => {
	const [isOpen, setIsOpen] = useState(false);

	const { mutate: deleteLoc, isLoading: deleteLocLoading } = useLibraryMutation(
		'locations.delete',
		{
			onSuccess: () => {
				onSubmit?.();
			},
			onSettled: () => {
				// Close dialog
				setIsOpen(false);
			}
		}
	);
	return (
		<Dialog
			isVisible={isOpen}
			setIsVisible={setIsOpen}
			title="Delete Location"
			description="Deleting a location will also remove all files associated with it from the Spacedrive database, the files themselves will not be deleted."
			ctaLabel="Delete"
			ctaAction={() => deleteLoc(locationId)}
			loading={deleteLocLoading}
			trigger={children}
			ctaDanger
		/>
	);
};

export default DeleteLocationDialog;
