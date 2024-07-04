/* eslint-disable no-restricted-imports */
import { CheckCircle, Info, WarningCircle } from 'phosphor-react-native';
import { Text, View } from 'react-native';
import Toast, { ToastConfig } from 'react-native-toast-message';
import { tw } from '~/lib/tailwind';

const baseStyles =
	'max-w-[340px] flex-row gap-1 items-center justify-center overflow-hidden rounded-md border p-3 shadow-lg bg-app-input border-app-inputborder';
const containerStyle = 'flex-row items-start gap-1.5';

const toastConfig: ToastConfig = {
	success: ({ text1, ...rest }) => (
		<View style={tw.style(baseStyles)}>
			<View style={tw.style(containerStyle)}>
				<CheckCircle size={20} weight="fill" color={tw.color('text-green-500')} />
				<Text
					style={tw`self-center text-left text-sm font-medium text-ink`}
					numberOfLines={3}
				>
					{text1}
				</Text>
			</View>
		</View>
	),
	error: ({ text1, ...rest }) => (
		<View style={tw.style(baseStyles)}>
			<View style={tw.style(containerStyle)}>
				<WarningCircle size={20} weight="fill" color={tw.color('text-red-500')} />
				<Text
					style={tw`self-center text-left text-sm font-medium text-ink`}
					numberOfLines={3}
				>
					{text1}
				</Text>
			</View>
		</View>
	),
	info: ({ text1, ...rest }) => (
		<View style={tw.style(baseStyles)}>
			<View style={tw.style(containerStyle)}>
				<Info size={20} weight="fill" color={tw.color('text-accent')} />
				<Text
					style={tw`self-center text-left text-sm font-medium text-ink`}
					numberOfLines={3}
				>
					{text1}
				</Text>
			</View>
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
