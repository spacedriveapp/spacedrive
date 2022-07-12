import create from 'zustand';

export interface Toast {
	title: string;
	subtitle: string;
	payload: ToastPayload;
}

export interface PairingRequest {
	id: string;
	name: string;
}

export type ToastPayload =
	| {
			type: 'pairingRequest';
			data: PairingRequest;
	  }
	| { type: 'noaction' };

export const useToastNotificationsStore = create<{
	toasts: Toast[];
	addToast: (toast: Toast) => void;
}>((set) => ({
	toasts: [],
	addToast: (toast: Toast) => set((state) => ({ toasts: [toast, ...state.toasts] }))
}));
