import { BottomSheetModal } from '@gorhom/bottom-sheet';
import React from 'react';
import { proxy, ref } from 'valtio';

import { FilePath } from '../types/bindings';

export const fileModalStore = proxy({
	fileRef: ref(React.createRef<BottomSheetModal>()),
	data: null as FilePath | null,
	setData: (data: FilePath) => {
		fileModalStore.data = data;
	}
});
