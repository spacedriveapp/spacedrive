import { createRef } from 'react';
import { proxy, ref, useSnapshot } from 'valtio';
import { ExplorerItem } from '@sd/client';
import { ModalRef } from '~/components/layout/Modal';

export const actionsModalStore = proxy({
	modalRef: ref(createRef<ModalRef>()),
	data: null as ExplorerItem | null,
	setData: (data: ExplorerItem) => {
		actionsModalStore.data = data;
	}
});

export function useActionsModalStore() {
	return useSnapshot(actionsModalStore);
}
