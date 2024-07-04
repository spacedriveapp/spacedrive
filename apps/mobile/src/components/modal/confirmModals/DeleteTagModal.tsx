import { useRef } from 'react';
import { useLibraryMutation, usePlausibleEvent, useRspcLibraryContext } from '@sd/client';
import { ConfirmModal, ModalRef } from '~/components/layout/Modal';
import { toast } from '~/components/primitive/Toast';

type Props = {
	tagId: number;
	onSubmit?: () => void;
	trigger: React.ReactNode;
	triggerStyle?: string;
};

const DeleteTagModal = ({ trigger, onSubmit, tagId, triggerStyle }: Props) => {
	const modalRef = useRef<ModalRef>(null);
	const rspc = useRspcLibraryContext();
	const submitPlausibleEvent = usePlausibleEvent();

	const { mutate: deleteTag, isLoading: deleteTagLoading } = useLibraryMutation('tags.delete', {
		onSuccess: () => {
			submitPlausibleEvent({ event: { type: 'tagDelete' } });
			onSubmit?.();
			rspc.queryClient.invalidateQueries(['tags.list']);
			toast.success('Tag deleted successfully');
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
			triggerStyle={triggerStyle}
			ctaDanger
		/>
	);
};

export default DeleteTagModal;
