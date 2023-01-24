import {
	BottomSheetBackdrop,
	BottomSheetBackdropProps,
	BottomSheetHandle,
	BottomSheetHandleProps,
	BottomSheetModal,
	BottomSheetModalProps,
	BottomSheetScrollView
} from '@gorhom/bottom-sheet';
import { X } from 'phosphor-react-native';
import { forwardRef } from 'react';
import { Pressable, Text } from 'react-native';
import useForwardedRef from '~/hooks/useForwardedRef';
import tw from '~/lib/tailwind';

const ModalBackdrop = (props: BottomSheetBackdropProps) => (
	<BottomSheetBackdrop {...props} appearsOnIndex={0} disappearsOnIndex={-1} opacity={0.75} />
);

interface ModalHandle extends BottomSheetHandleProps {
	hideCloseButton: boolean;
	modalRef: React.RefObject<BottomSheetModal>;
}

const ModalHandle = (props: ModalHandle) => (
	<BottomSheetHandle
		{...props}
		style={tw`bg-app rounded-t-2xl items-end`}
		indicatorStyle={tw`bg-app-highlight/60`}
	>
		{!props.hideCloseButton && (
			<Pressable
				onPress={() => props.modalRef.current.close()}
				style={tw`absolute top-4 right-3 w-7 h-7 items-center justify-center bg-app-button rounded-full mr-1`}
			>
				<X size={16} color="white" weight="bold" />
			</Pressable>
		)}
	</BottomSheetHandle>
);

export type ModalRef = BottomSheetModal;

interface ModalProps extends BottomSheetModalProps {
	children: React.ReactNode;
	title?: string;
	hideCloseButton?: boolean;
}

export const Modal = forwardRef<ModalRef, ModalProps>((props, ref) => {
	const { children, title, hideCloseButton = false, ...otherProps } = props;

	const modalRef = useForwardedRef(ref);

	return (
		<BottomSheetModal
			ref={modalRef}
			backgroundStyle={tw`bg-app`}
			backdropComponent={ModalBackdrop}
			handleComponent={(props) => ModalHandle({ modalRef, hideCloseButton, ...props })}
			{...otherProps}
		>
			{title && <Text style={tw`text-ink font-bold text-sm text-center`}>{title}</Text>}
			{children}
		</BottomSheetModal>
	);
});

export const ModalScrollView = BottomSheetScrollView;
