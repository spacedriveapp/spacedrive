/* eslint-disable no-restricted-imports */
import Toast, { BaseToast, ErrorToast, ToastConfig } from 'react-native-toast-message';

const toastConfig: ToastConfig = {};

function toast(type: 'success' | 'error' | 'info', text1: string, text2?: string) {
	Toast.show({ type, text1, text2 });
}

export { toast, Toast, toastConfig };
