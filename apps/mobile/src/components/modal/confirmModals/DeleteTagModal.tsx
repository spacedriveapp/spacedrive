import { useRef } from 'react';
import { useLibraryMutation, usePlausibleEvent } from '@sd/client';
import { ConfirmModal, ModalRef } from '~/components/layout/Modal';

type Props = {
	tagId: number;
	onSubmit?: () => void;
	trigger: React.ReactNode;
};

const DeleteTagModal = ({ trigger, onSubmit, tagId }: Props) => {
	const modalRef = useRef<ModalRef>(null);

	const submitPlausibleEvent = usePlausibleEvent();

	const { mutate: deleteTag, isLoading: deleteTagLoading } = useLibraryMutation('tags.delete', {
		onSuccess: () => {
			submitPlausibleEvent({ event: { type: 'tagDelete' } });
			onSubmit?.();
		},
		onSettled: () => {
			modalRef.current?.close();
		}
	});

	return (
		<ConfirmModal
			ref={modalRef}
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
