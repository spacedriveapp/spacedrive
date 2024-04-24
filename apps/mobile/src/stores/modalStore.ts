import { createRef } from 'react';
import { proxy, ref, useSnapshot } from 'valtio';
import { ExplorerItem } from '@sd/client';
import { ModalRef } from '~/components/layout/Modal';

const store = proxy({
	modalRef: ref(createRef<ModalRef>()),
	data: null as ExplorerItem | null,
	setData: (data: ExplorerItem) => {
		store.data = data;
	}
});

/** for reading */
export const useActionsModalStore = () => useSnapshot(store);
/** for writing */
export const getActionsModalStore = () => store;
