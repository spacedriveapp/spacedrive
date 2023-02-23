import { useRef } from 'react';
import { useLibraryMutation } from '@sd/client';
import { ConfirmModal, ModalRef } from '~/components/layout/Modal';

type Props = {
	tagId: number;
	onSubmit?: () => void;
	trigger: React.ReactNode;
};

const DeleteTagModal = ({ trigger, onSubmit, tagId }: Props) => {
	const modalRef = useRef<ModalRef>();

	const { mutate: deleteTag, isLoading: deleteTagLoading } = useLibraryMutation('tags.delete', {
		onSuccess: () => {
			onSubmit?.();
		},
		onSettled: () => {
			modalRef.current.close();
		}
	});

	return (
		<ConfirmModal
			title="Delete Tag"
			description="Are you sure you want to delete this tag? This cannot be undone and tagged files will be unlinked."
			ctaLabel="Delete"
			ctaAction={() => deleteTag(tagId)}
			loading={deleteTagLoading}
			trigger={trigger}
			ctaDanger
		/>
	);
};

export default DeleteTagModal;
