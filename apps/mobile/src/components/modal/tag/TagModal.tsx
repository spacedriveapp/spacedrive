import { forwardRef, useRef } from 'react';
import { Text, View } from 'react-native';
import { Tag } from '@sd/client';
import { Modal, ModalRef } from '~/components/layout/Modal';
import { Button, FakeButton } from '~/components/primitive/Button';
import useForwardedRef from '~/hooks/useForwardedRef';
import { tw } from '~/lib/tailwind';

import DeleteTagModal from '../confirmModals/DeleteTagModal';
import UpdateTagModal from './UpdateTagModal';

interface Props {
	tag: Tag;
}

export const TagModal = forwardRef<ModalRef, Props>(({ tag }, ref) => {
	const modalRef = useForwardedRef(ref);
	const editTagModalRef = useRef<ModalRef>(null);
	return (
		<Modal ref={modalRef} snapPoints={['17']} title="Tag actions">
			<View style={tw`mt-4 flex-row gap-5 px-6`}>
				<Button
					onPress={() => editTagModalRef.current?.present()}
					style={tw`flex-1 px-0`}
					variant="gray"
				>
					<Text style={tw`text-sm font-medium text-ink`}>Edit</Text>
				</Button>
				<DeleteTagModal
					tagId={tag.id}
					triggerStyle="flex-1"
					trigger={
						<FakeButton variant="danger">
							<Text style={tw`text-sm font-medium text-ink`}>Delete</Text>
						</FakeButton>
					}
				/>
				<UpdateTagModal ref={editTagModalRef} tag={tag} />
			</View>
		</Modal>
	);
});
