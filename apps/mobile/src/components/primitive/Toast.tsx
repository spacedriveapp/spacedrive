/* eslint-disable no-restricted-imports */
import { Text, View } from 'react-native';
import Toast, { ToastConfig } from 'react-native-toast-message';
import { tw } from '~/lib/tailwind';

const baseStyles = 'w-[340px] flex-row overflow-hidden rounded-md border p-3 shadow-lg';

const toastConfig: ToastConfig = {
	success: ({ text1, ...rest }) => (
		<View style={tw.style(baseStyles, 'border-app-line bg-app-darkBox/90 ')}>
			<Text style={tw`text-sm font-medium text-ink`} numberOfLines={3}>
				{text1}
			</Text>
		</View>
	),
	error: ({ text1, ...rest }) => (
		<View style={tw.style(baseStyles, 'border-red-500 bg-red-500/90')}>
			<Text style={tw`text-sm font-medium text-ink`} numberOfLines={3}>
				{text1}
			</Text>
		</View>
	),
	info: ({ text1, ...rest }) => (
		<View style={tw.style(baseStyles, 'border-app-line bg-app-darkBox/90')}>
			<Text style={tw`text-sm font-medium text-ink`} numberOfLines={3}>
				{text1}
			</Text>
		</View>
	)
};

function showToast({ text, type }: { type: 'success' | 'error' | 'info'; text: string }) {
	const visibilityTime = 3000;
	const topOffset = 60;
	Toast.show({ type, text1: text, visibilityTime, topOffset });
}

const toast: {
	success: (text: string) => void;
	error: (text: string) => void;
	info: (text: string) => void;
} = {
	success: function (text: string): void {
		showToast({ text, type: 'success' });
	},
	error: function (text: string): void {
		showToast({ text, type: 'error' });
	},
	info: function (text: string): void {
		showToast({ text, type: 'info' });
	}
};

export { Toast, toast, toastConfig };
