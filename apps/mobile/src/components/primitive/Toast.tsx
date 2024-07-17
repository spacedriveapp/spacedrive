/* eslint-disable no-restricted-imports */
import { CheckCircle, Info, WarningCircle } from 'phosphor-react-native';
import { PropsWithChildren, useEffect, useRef, useState } from 'react';
import {
	LayoutAnimation,
	Platform,
	Pressable,
	Text,
	TouchableOpacity,
	UIManager,
	View
} from 'react-native';
import Toast, { ToastConfig } from 'react-native-toast-message';
import { tw, twStyle } from '~/lib/tailwind';

const baseStyles =
	'max-w-[340px] flex-row gap-1 items-center justify-center overflow-hidden rounded-md border p-3 shadow-lg bg-app-input border-app-inputborder';
const containerStyle = 'flex-row items-start gap-1.5';

const MAX_LINES = 3;

const CollapsibleText = ({ children }: PropsWithChildren) => {
	const [expanded, setExpanded] = useState(false);
	const [showButton, setShowButton] = useState(false);
	const [canExpand, setcanExpand] = useState(false);
	const textRef = useRef<Text>(null);

	//this makes sure the animation works and runs on Android
	if (Platform.OS === 'android' && UIManager.setLayoutAnimationEnabledExperimental) {
		UIManager.setLayoutAnimationEnabledExperimental(true);
	}

	useEffect(() => {
		if (textRef.current) {
			textRef.current.measure((x, y, width, height, pageX, pageY) => {
				const lineHeight = 20; // Customize this value according to your text line height
				if (height >= lineHeight * MAX_LINES) {
					setcanExpand(true);
					setShowButton(true);
				}
			});
		}
	}, []);

	const handleToggle = () => {
		LayoutAnimation.configureNext(LayoutAnimation.Presets.easeInEaseOut);
		setExpanded(!expanded);
	};

	return (
		<View style={twStyle(canExpand ? 'flex-1' : undefined)}>
			<Text
				ref={textRef}
				style={tw`text-left text-sm text-ink`}
				numberOfLines={expanded ? undefined : MAX_LINES}
			>
				{children}
			</Text>
			{showButton && (
				<TouchableOpacity onPress={handleToggle}>
					<Text style={tw`mt-1.5 font-medium text-blue-500`}>
						{expanded ? 'Read less' : 'Read more'}
					</Text>
				</TouchableOpacity>
			)}
		</View>
	);
};

const toastConfig: ToastConfig = {
	success: ({ text1, onPress, ...rest }) => {
		return (
			<Pressable onPress={onPress}>
				<View style={tw.style(baseStyles)}>
					<View style={tw.style(containerStyle)}>
						<View>
							<CheckCircle
								size={20}
								weight="fill"
								color={tw.color('text-green-500')}
							/>
						</View>
						<CollapsibleText>{text1}</CollapsibleText>
					</View>
				</View>
			</Pressable>
		);
	},
	error: ({ text1, onPress, ...rest }) => (
		<Pressable onPress={onPress}>
			<View style={tw.style(baseStyles)}>
				<View style={tw.style(containerStyle)}>
					<View>
						<WarningCircle size={20} weight="fill" color={tw.color('text-red-500')} />
					</View>
					<CollapsibleText>{text1}</CollapsibleText>
				</View>
			</View>
		</Pressable>
	),
	info: ({ text1, onPress, ...rest }) => (
		<Pressable onPress={onPress}>
			<View style={tw.style(baseStyles)}>
				<View style={tw.style(containerStyle)}>
					<View>
						<Info size={20} weight="fill" color={tw.color('text-accent')} />
					</View>
					<CollapsibleText>{text1}</CollapsibleText>
				</View>
			</View>
		</Pressable>
	)
};

function showToast({
	text,
	onPress,
	type
}: {
	type: 'success' | 'error' | 'info';
	text: string;
	onPress?: () => void;
}): void {
	const visibilityTime = 8000;
	const topOffset = 60;
	Toast.show({ type, text1: text, onPress, visibilityTime, topOffset });
}

const toast: {
	success: (text: string, onPress?: () => void) => void;
	error: (text: string, onPress?: () => void) => void;
	info: (text: string, onPress?: () => void) => void;
} = {
	success: function (text, onPress): void {
		showToast({ text, onPress, type: 'success' });
	},
	error: function (text, onPress): void {
		showToast({ text, onPress, type: 'error' });
	},
	info: function (text, onPress): void {
		showToast({ text, onPress, type: 'info' });
	}
};

export { Toast, toast, toastConfig };
