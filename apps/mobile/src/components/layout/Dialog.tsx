import { MotiView } from 'moti';
import { ReactNode, useState } from 'react';
import { KeyboardAvoidingView, Modal, Platform, Pressable, Text, View } from 'react-native';
import tw from '~/lib/tailwind';

import { PulseAnimation } from '../animation/lottie';
import { Button } from '../primitive/Button';

type DialogProps = {
	title: string;
	description?: string;
	trigger?: ReactNode;
	/**
	 * if `true`, dialog will be visible when mounted.
	 * It can be used when trigger is not provided and/or you need to open the dialog programmatically
	 */
	isVisible?: boolean;
	/**
	 * Like above, it will override the default dialog state for opening/closing the dialog.
	 * It can be used to control dialog state from outside
	 */
	setIsVisible?: (v: boolean) => void;
	children?: ReactNode;
	ctaAction?: () => void;
	ctaLabel?: string;
	ctaDanger?: boolean;
	ctaDisabled?: boolean;
	loading?: boolean;
	/**
	 * Disables backdrop press to close the modal.
	 */
	disableBackdropClose?: boolean;
	/**
	 * Triggered when the dialog is closed (either by backdrop or the close button)
	 */
	onClose?: () => void;
};

const Dialog = (props: DialogProps) => {
	const [visible, setVisible] = useState(props.isVisible ?? false);

	function handleCloseDialog() {
		props.setIsVisible ? props.setIsVisible(false) : setVisible(false);
		// Cool undefined check
		props.onClose?.();
	}

	return (
		<View>
			{props.trigger && (
				<Pressable
					onPress={() => (props.setIsVisible ? props.setIsVisible(true) : setVisible(true))}
				>
					{props.trigger}
				</Pressable>
			)}
			<Modal renderToHardwareTextureAndroid transparent visible={props.isVisible ?? visible}>
				{/* Backdrop */}
				<Pressable
					style={tw`bg-black bg-opacity-50 absolute inset-0`}
					onPress={handleCloseDialog}
					disabled={props.disableBackdropClose || props.loading}
				/>
				{/* Content */}
				<KeyboardAvoidingView
					pointerEvents="box-none"
					behavior={Platform.OS === 'ios' ? 'padding' : undefined}
					keyboardVerticalOffset={Platform.OS === 'ios' ? 40 : undefined}
					style={tw`flex-1 items-center justify-center`}
				>
					<MotiView
						from={{ translateY: 40 }}
						animate={{ translateY: 0 }}
						transition={{ type: 'timing', duration: 200 }}
					>
						<View
							style={tw`min-w-[360px] max-w-[380px] rounded-md bg-gray-650 border border-gray-550 shadow-md overflow-hidden`}
						>
							<View style={tw`p-5`}>
								{/* Title */}
								<Text style={tw`font-bold text-white text-base`}>{props.title}</Text>
								{/* Description */}
								{props.description && (
									<Text style={tw`text-sm text-gray-300 mt-2 leading-normal`}>
										{props.description}
									</Text>
								)}
								{/* Children */}
								<View style={tw`mt-3`}>{props.children}</View>
							</View>
							{/* Actions */}
							<View
								style={tw`flex flex-row items-center px-3 py-3 bg-gray-600 border-t border-gray-550`}
							>
								{props.loading && <PulseAnimation style={tw`h-7`} />}
								<View style={tw`flex-grow`} />
								<Button
									variant="dark_gray"
									size="md"
									disabled={props.loading} // Disables Close button if loading
									onPress={handleCloseDialog}
								>
									<Text style={tw`text-white text-sm`}>Close</Text>
								</Button>
								{props.ctaAction && (
									<Button
										style={tw`ml-2`}
										variant={props.ctaDanger ? 'danger' : 'primary'}
										size="md"
										onPress={props.ctaAction}
										disabled={props.ctaDisabled || props.loading}
									>
										<Text style={tw`text-white text-sm`}>{props.ctaLabel}</Text>
									</Button>
								)}
							</View>
						</View>
					</MotiView>
				</KeyboardAvoidingView>
			</Modal>
		</View>
	);
};

export default Dialog;
