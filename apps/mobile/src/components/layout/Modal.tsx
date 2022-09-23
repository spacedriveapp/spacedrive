import {
	BottomSheetBackdrop,
	BottomSheetBackdropProps,
	BottomSheetHandle,
	BottomSheetHandleProps
} from '@gorhom/bottom-sheet';
import tw from '~/lib/tailwind';

export const ModalBackdrop = (props: BottomSheetBackdropProps) => {
	return (
		<BottomSheetBackdrop {...props} appearsOnIndex={0} disappearsOnIndex={-1} opacity={0.75} />
	);
};

export const ModalHandle = (props: BottomSheetHandleProps) => {
	return (
		<BottomSheetHandle
			{...props}
			style={tw`bg-gray-600 rounded-t-xl`}
			indicatorStyle={tw`bg-gray-550`}
		/>
	);
};
