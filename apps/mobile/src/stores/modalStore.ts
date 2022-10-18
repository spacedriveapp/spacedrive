import { BottomSheetModalMethods } from '@gorhom/bottom-sheet/lib/typescript/types';
import { RefObject } from 'react';
import { proxy } from 'valtio';

import { FilePath } from '../types/bindings';

export const fileModalStore = proxy({
	fileRef: null as RefObject<BottomSheetModalMethods>,
	data: null as FilePath | null,
	setData: (data: FilePath) => {
		fileModalStore.data = data;
	}
});
