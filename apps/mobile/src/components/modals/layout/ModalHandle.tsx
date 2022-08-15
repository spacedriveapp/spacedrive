import { BottomSheetHandle, BottomSheetHandleProps } from '@gorhom/bottom-sheet';
import React from 'react';

import tw from '../../../lib/tailwind';

const ModalHandle = (props: BottomSheetHandleProps) => {
	return (
		<BottomSheetHandle
			{...props}
			style={tw`bg-gray-600 rounded-t-xl`}
			indicatorStyle={tw`bg-gray-500`}
		/>
	);
};

export default ModalHandle;
