import {
	BottomSheetBackdrop,
	BottomSheetBackdropProps,
	BottomSheetHandle,
	BottomSheetHandleProps,
	BottomSheetModal,
	BottomSheetModalProps
} from '@gorhom/bottom-sheet';
import { BottomSheetModalMethods } from '@gorhom/bottom-sheet/lib/typescript/types';
import React, { forwardRef } from 'react';
import tw from '~/lib/tailwind';

const ModalBackdrop = (props: BottomSheetBackdropProps) => (
	<BottomSheetBackdrop {...props} appearsOnIndex={0} disappearsOnIndex={-1} opacity={0.75} />
);

const ModalHandle = (props: BottomSheetHandleProps) => (
	<BottomSheetHandle
		{...props}
		style={tw`bg-gray-600 rounded-t-xl`}
		indicatorStyle={tw`bg-gray-550`}
	/>
);

export const Modal = forwardRef<BottomSheetModalMethods, BottomSheetModalProps>((props, ref) => (
	<BottomSheetModal
		ref={ref}
		backdropComponent={ModalBackdrop}
		handleComponent={ModalHandle}
		backgroundStyle={tw.style('bg-gray-600')}
		{...props}
	/>
));
