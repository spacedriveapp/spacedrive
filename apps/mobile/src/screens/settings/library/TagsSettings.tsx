import { useRef } from 'react';
import { ModalRef } from '~/components/layout/Modal';
import CreateTagModal from '~/components/modal/tag/CreateTagModal';
import Tags from '~/screens/Tags';

const TagsSettingsScreen = () => {
	const modalRef = useRef<ModalRef>(null);

	return (
		<>
			<Tags viewStyle="list" />
			<CreateTagModal ref={modalRef} />
		</>
	);
};

export default TagsSettingsScreen;
