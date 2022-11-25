import { BottomSheetModal } from '@gorhom/bottom-sheet';
import { ExplorerItem } from '@sd/client';
import { createRef } from 'react';
import { proxy, ref, useSnapshot } from 'valtio';

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
