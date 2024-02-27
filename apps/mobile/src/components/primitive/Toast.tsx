/* eslint-disable no-restricted-imports */
import { Text, View } from 'react-native';
import Toast, { ToastConfig } from 'react-native-toast-message';
import { tw } from '~/lib/tailwind';

// TODO:
// - Expand toast on press to show full message if it's too long
// - Add a onPress option
// - Add leading icon & trailing icon

const toastConfig: ToastConfig = {
	success: ({ text1, ...rest }) => (
		<View
			style={tw`w-[340px] flex-row overflow-hidden rounded-md border border-app-line bg-app-darkBox/90 p-3 shadow-lg`}
		>
			<Text style={tw`text-sm font-medium text-ink`} numberOfLines={3}>
				{text1}
			</Text>
		</View>
	),
	error: ({ text1, ...rest }) => (
		<View
			style={tw`border-app-red bg-app-red/90 w-[340px] flex-row overflow-hidden rounded-md border p-3 shadow-lg`}
		>
			<Text style={tw`text-sm font-medium text-ink`} numberOfLines={3}>
				{text1}
			</Text>
		</View>
	)
};

function toast({ text, type }: { type: 'success' | 'error' | 'info'; text: string }) {
	Toast.show({ type, text1: text, visibilityTime: 3000, topOffset: 60 });
}

export { Toast, toast, toastConfig };
