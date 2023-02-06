import {
	BottomSheetBackdrop,
	BottomSheetBackdropProps,
	BottomSheetHandle,
	BottomSheetHandleProps,
	BottomSheetModal,
	BottomSheetModalProps
} from '@gorhom/bottom-sheet';
import { BottomSheetModalMethods } from '@gorhom/bottom-sheet/lib/typescript/types';
import { forwardRef } from 'react';
import tw from '~/lib/tailwind';

const ModalBackdrop = (props: BottomSheetBackdropProps) => (
	<BottomSheetBackdrop {...props} appearsOnIndex={0} disappearsOnIndex={-1} opacity={0.75} />
);

const ModalHandle = (props: BottomSheetHandleProps) => (
	<BottomSheetHandle
		{...props}
		style={tw`rounded-t-xl bg-app`}
		indicatorStyle={tw`bg-app-highlight`}
	/>
);

export const Modal = forwardRef<BottomSheetModalMethods, BottomSheetModalProps>((props, ref) => (
	<BottomSheetModal
		ref={ref}
		backdropComponent={ModalBackdrop}
		handleComponent={ModalHandle}
		backgroundStyle={tw`bg-app`}
		{...props}
	/>
));
