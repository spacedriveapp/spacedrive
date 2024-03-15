import { Pen, Trash } from 'phosphor-react-native';
import { useRef } from 'react';
import { Animated } from 'react-native';
import { Swipeable } from 'react-native-gesture-handler';
import { Tag } from '@sd/client';
import { tw } from '~/lib/tailwind';

import { ModalRef } from '../layout/Modal';
import DeleteTagModal from '../modal/confirmModals/DeleteTagModal';
import UpdateTagModal from '../modal/tag/UpdateTagModal';
import { AnimatedButton, FakeButton } from '../primitive/Button';

interface Props {
	progress: Animated.AnimatedInterpolation<number>;
	swipeable: Swipeable;
	tag: Tag;
}

const RightActions = ({ progress, swipeable, tag }: Props) => {
	const modalRef = useRef<ModalRef>(null);
	const translate = progress.interpolate({
		inputRange: [0, 1],
		outputRange: [100, 0],
		extrapolate: 'clamp'
	});

	return (
		<Animated.View
			style={[
				tw`ml-0 flex flex-row items-center`,
				{ transform: [{ translateX: translate }] }
			]}
		>
			<UpdateTagModal tag={tag} ref={modalRef} onSubmit={() => swipeable.close()} />
			<AnimatedButton onPress={() => modalRef.current?.present()}>
				<Pen size={18} color="white" />
			</AnimatedButton>
			<DeleteTagModal
				tagId={tag.id}
				trigger={
					<FakeButton style={tw`mx-2 border-app-lightborder bg-app-button`}>
						<Trash size={18} color="white" />
					</FakeButton>
				}
			/>
		</Animated.View>
	);
};

export default RightActions;
