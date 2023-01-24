import { useState } from 'react';
import { useLibraryMutation } from '@sd/client';
import Dialog from '~/components/layout/Dialog';

type Props = {
	tagId: number;
	onSubmit?: () => void;
	children: React.ReactNode;
};

const DeleteTagDialog = ({ children, onSubmit, tagId }: Props) => {
	const [isOpen, setIsOpen] = useState(false);

	const { mutate: deleteTag, isLoading: deleteTagLoading } = useLibraryMutation('tags.delete', {
		onSuccess: () => {
			onSubmit?.();
		},
		onSettled: () => {
			// Close dialog
			setIsOpen(false);
		}
	});
	return (
		<Dialog
			isVisible={isOpen}
			setIsVisible={setIsOpen}
			title="Delete Tag"
			description="Are you sure you want to delete this tag? This cannot be undone and tagged files will be unlinked."
			ctaLabel="Delete"
			ctaAction={() => deleteTag(tagId)}
			loading={deleteTagLoading}
			trigger={children}
			ctaDanger
		/>
	);
};

export default DeleteTagDialog;
