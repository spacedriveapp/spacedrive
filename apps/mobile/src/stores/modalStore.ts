import { createRef } from 'react';
import { proxy, ref, useSnapshot } from 'valtio';
import { ExplorerItem } from '@sd/client';
import { ModalRef } from '~/components/layout/Modal';

export const inspectorModalStore = proxy({
	modalRef: ref(createRef<ModalRef>()),
	data: null as ExplorerItem | null,
	setData: (data: ExplorerItem) => {
		inspectorModalStore.data = data;
	}
});

export function useInspectorModalStore() {
	return useSnapshot(inspectorModalStore);
}
