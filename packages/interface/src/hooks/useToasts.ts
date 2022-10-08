import { proxy, useSnapshot } from 'valtio';

interface Toast {
	id: string;
	title: string;
	subtitle?: string;
	duration?: number;
	actionButton?: {
		text: string;
		onClick: () => void;
	};
}

const state = proxy({
	toasts: [] as Toast[]
});

const randomId = () => Math.random().toString(36).slice(2);

export function useToasts() {
	return {
		toasts: useSnapshot(state).toasts,
		addToast: (toast: Omit<Toast, 'id'>) => {
			state.toasts.push({
				id: randomId(),
				...toast
			});
		},
		removeToast: (toast: Toast | string) => {
			const id = typeof toast === 'string' ? toast : toast.id;
			state.toasts = state.toasts.filter((t) => t.id !== id);
		}
	};
}
