import {
	BottomSheetBackdrop,
	BottomSheetBackdropProps,
	BottomSheetFlatList,
	BottomSheetHandle,
	BottomSheetHandleProps,
	BottomSheetModal,
	BottomSheetModalProps,
	BottomSheetScrollView
} from '@gorhom/bottom-sheet';
import { X } from 'phosphor-react-native';
import { forwardRef, ReactNode } from 'react';
import { Platform, Pressable, Text, View } from 'react-native';
import useForwardedRef from '~/hooks/useForwardedRef';
import { tw, twStyle } from '~/lib/tailwind';

import { Button } from '../primitive/Button';

const ModalBackdrop = (props: BottomSheetBackdropProps) => (
	<BottomSheetBackdrop {...props} appearsOnIndex={0} disappearsOnIndex={-1} opacity={0.75} />
);

interface ModalHandle extends BottomSheetHandleProps {
	showCloseButton: boolean;
	modalRef: React.RefObject<BottomSheetModal>;
}

const ModalHandle = (props: ModalHandle) => (
	<BottomSheetHandle
		{...props}
		style={tw`items-end rounded-t-2xl bg-app-modal`}
		indicatorStyle={tw`bg-app-lightborder`}
	>
		{props.showCloseButton && (
			<Pressable
				onPress={() => props.modalRef.current?.close()}
				style={tw`absolute right-4 top-5 h-7 w-7 items-center justify-center rounded-full bg-app-button`}
			>
				<X size={16} color="white" weight="bold" />
			</Pressable>
		)}
	</BottomSheetHandle>
);

export type ModalRef = BottomSheetModal;

interface ModalProps extends BottomSheetModalProps {
	children: React.ReactNode;
	title?: string;
	description?: string;
	showCloseButton?: boolean;
}

export const Modal = forwardRef<ModalRef, ModalProps>((props, ref) => {
	const { children, title, description, showCloseButton = false, ...otherProps } = props;

	const modalRef = useForwardedRef(ref);

	return (
		<BottomSheetModal
			ref={modalRef}
			backgroundStyle={tw`bg-app-modal`}
			backdropComponent={ModalBackdrop}
			handleComponent={(props) => ModalHandle({ modalRef, showCloseButton, ...props })}
			// Overriding the default value for iOS to fix Maestro issue.
			// https://github.com/app-dev-inc/maestro/issues/1493
			accessible={Platform.select({
				// setting it to false on Android seems to cause issues with TalkBack instead
				ios: false
			})}
			{...otherProps}
		>
			{title && <Text style={tw`text-center text-base font-medium text-ink`}>{title}</Text>}
			{props.description && (
				<Text style={tw`px-4 py-3 text-sm text-ink-dull`}>{props.description}</Text>
			)}
			{children}
		</BottomSheetModal>
	);
});

export const ModalScrollView = BottomSheetScrollView;
export const ModalFlatlist = BottomSheetFlatList;

type ConfirmModalProps = {
	title: string;
	description?: string;
	ctaAction?: () => void;
	ctaLabel: string;
	ctaDanger?: boolean;
	ctaDisabled?: boolean;
	loading?: boolean;
	/**
	 * Disables backdrop press to close the modal.
	 */
	disableBackdropClose?: boolean;
	/**
	 * Children will be rendered below the description and above the CTA button.
	 */
	children?: React.ReactNode;
	snapPoints?: (string | number)[];
	/**
	 * Trigger to open the modal.
	 * You can also use ref to open the modal
	 */
	trigger?: ReactNode;
	triggerStyle?: string;
};

// TODO: Add loading state
// Drop-in replacement for Dialog, can be used to get confirmation from the user, e.g. deleting a library
export const ConfirmModal = forwardRef<ModalRef, ConfirmModalProps>((props, ref) => {
	const modalRef = useForwardedRef(ref);

	return (
		<>
			{props.trigger && (
				<Pressable
					style={twStyle(props.triggerStyle)}
					onPress={() => modalRef.current?.present()}
				>
					{props.trigger}
				</Pressable>
			)}
			<BottomSheetModal
				ref={modalRef}
				backgroundStyle={tw`bg-app-modal`}
				backdropComponent={ModalBackdrop}
				handleComponent={(props) =>
					ModalHandle({ modalRef, showCloseButton: false, ...props })
				}
				snapPoints={props.snapPoints ?? ['25']}
			>
				{/* Title */}
				{props.title && (
					<Text style={tw`text-center text-base font-medium text-ink`}>
						{props.title}
					</Text>
				)}
				<View style={tw`mt-4 px-6`}>
					{/* Description */}
					{props.description && (
						<Text style={tw`text-sm text-ink-dull`}>{props.description}</Text>
					)}
					{/* Children */}
					{props.children && props.children}
					{/* Buttons */}
					<View style={tw`flex flex-row pt-5`}>
						<Button
							variant="gray"
							style={tw`flex-1`}
							size="lg"
							disabled={props.loading} // Disables Close button if loading
							onPress={() => modalRef.current?.close()}
						>
							<Text style={tw`text-sm font-medium text-ink`}>Close</Text>
						</Button>
						{props.ctaAction && (
							<Button
								style={tw`ml-4 flex-1`}
								variant={props.ctaDanger ? 'danger' : 'accent'}
								size="lg"
								onPress={props.ctaAction}
								disabled={props.ctaDisabled || props.loading}
							>
								<Text style={tw`text-sm font-medium text-ink`}>
									{props.ctaLabel}
								</Text>
							</Button>
						)}
					</View>
				</View>
			</BottomSheetModal>
		</>
	);
});
