import { createRef } from 'react';
import { proxy, ref, useSnapshot } from 'valtio';
import { ExplorerItem } from '@sd/client';
import { ModalRef } from '~/components/layout/Modal';

export const fileModalStore = proxy({
	fileRef: ref(createRef<ModalRef>()),
	data: null as ExplorerItem | null,
	setData: (data: ExplorerItem) => {
		fileModalStore.data = data;
	}
});

export function useFileModalStore() {
	return useSnapshot(fileModalStore);
}
