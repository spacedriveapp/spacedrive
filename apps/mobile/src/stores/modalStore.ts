import { BottomSheetModal } from '@gorhom/bottom-sheet';
import { createRef } from 'react';
import { proxy, ref, useSnapshot } from 'valtio';
import { ExplorerItem } from '@sd/client';

export const fileModalStore = proxy({
	fileRef: ref(createRef<BottomSheetModal>()),
	data: null as ExplorerItem | null,
	setData: (data: ExplorerItem) => {
		fileModalStore.data = data;
	}
});

export function useFileModalStore() {
	return useSnapshot(fileModalStore);
}
