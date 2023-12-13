import { useSnapshot } from 'valtio';
import { valtioPersist } from '@sd/client';

export const dismissibleNoticeStore = valtioPersist('dismissible-notice', {
	mediaView: false,
	gridView: false,
	listView: false,
	ephemeral: false,
	ephemeralMoveFiles: false
});

export function useDismissibleNoticeStore() {
	return useSnapshot(dismissibleNoticeStore);
}

export function getDismissibleNoticeStore() {
	return dismissibleNoticeStore;
}
