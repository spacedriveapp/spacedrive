import { BottomSheetBackdrop, BottomSheetBackdropProps } from '@gorhom/bottom-sheet';

const ModalBackdrop = (props: BottomSheetBackdropProps) => {
	return (
		<BottomSheetBackdrop {...props} appearsOnIndex={0} disappearsOnIndex={-1} opacity={0.75} />
	);
};

export default ModalBackdrop;
