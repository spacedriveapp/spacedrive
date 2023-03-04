import {
	BottomSheetBackdrop,
	BottomSheetBackdropProps,
	BottomSheetHandle,
	BottomSheetHandleProps,
	BottomSheetModal,
	BottomSheetModalProps,
	BottomSheetScrollView
} from '@gorhom/bottom-sheet';
import { X } from 'phosphor-react-native';
import { ReactNode, forwardRef } from 'react';
import { Pressable, Text, View } from 'react-native';
import useForwardedRef from '~/hooks/useForwardedRef';
import { tw } from '~/lib/tailwind';
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
		style={tw`bg-app items-end rounded-t-2xl`}
		indicatorStyle={tw`bg-app-highlight/60`}
	>
		{props.showCloseButton && (
			<Pressable
				onPress={() => props.modalRef.current?.close()}
				style={tw`bg-app-button absolute top-5 right-4 h-7 w-7 items-center justify-center rounded-full`}
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
	showCloseButton?: boolean;
}

export const Modal = forwardRef<ModalRef, ModalProps>((props, ref) => {
	const { children, title, showCloseButton = false, ...otherProps } = props;

	const modalRef = useForwardedRef(ref);

	return (
		<BottomSheetModal
			ref={modalRef}
			backgroundStyle={tw`bg-app`}
			backdropComponent={ModalBackdrop}
			handleComponent={(props) => ModalHandle({ modalRef, showCloseButton, ...props })}
			{...otherProps}
		>
			{title && <Text style={tw`text-ink text-center text-base font-medium`}>{title}</Text>}
			{children}
		</BottomSheetModal>
	);
});

export const ModalScrollView = BottomSheetScrollView;

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
};

// TODO: Add loading state
// Drop-in replacement for Dialog, can be used to get confirmation from the user, e.g. deleting a library
export const ConfirmModal = forwardRef<ModalRef, ConfirmModalProps>((props, ref) => {
	const modalRef = useForwardedRef(ref);

	return (
		<>
			{props.trigger && (
				<Pressable onPress={() => modalRef.current?.present()}>{props.trigger}</Pressable>
			)}
			<BottomSheetModal
				ref={modalRef}
				backgroundStyle={tw`bg-app`}
				backdropComponent={ModalBackdrop}
				handleComponent={(props) => ModalHandle({ modalRef, showCloseButton: false, ...props })}
				snapPoints={props.snapPoints ?? ['25%']}
			>
				{/* Title */}
				{props.title && (
					<Text style={tw`text-ink text-center text-base font-medium`}>{props.title}</Text>
				)}
				<View style={tw`mt-4 px-6`}>
					{/* Description */}
					{props.description && <Text style={tw`text-ink-dull text-sm`}>{props.description}</Text>}
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
							<Text style={tw`text-ink text-sm font-medium`}>Close</Text>
						</Button>
						{props.ctaAction && (
							<Button
								style={tw`ml-4 flex-1`}
								variant={props.ctaDanger ? 'danger' : 'accent'}
								size="lg"
								onPress={props.ctaAction}
								disabled={props.ctaDisabled || props.loading}
							>
								<Text style={tw`text-ink text-sm font-medium`}>{props.ctaLabel}</Text>
							</Button>
						)}
					</View>
				</View>
			</BottomSheetModal>
		</>
	);
});
