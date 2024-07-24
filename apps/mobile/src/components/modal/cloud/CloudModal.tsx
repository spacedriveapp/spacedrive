import { useNavigation } from '@react-navigation/native';
import React, { forwardRef } from 'react';
import { Text, View } from 'react-native';
import { Icon } from '~/components/icons/Icon';
import { Modal, ModalRef } from '~/components/layout/Modal';
import { Button } from '~/components/primitive/Button';
import useForwardedRef from '~/hooks/useForwardedRef';
import { tw } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/tabs/SettingsStack';

const CloudModal = forwardRef<ModalRef>((_, ref) => {
	const modalRef = useForwardedRef(ref);
	const navigation = useNavigation<SettingsStackScreenProps<'CloudSettings'>['navigation']>();
	return (
		<Modal showCloseButton ref={modalRef} snapPoints={['30']} title="Cloud Sync">
			<View style={tw`mx-auto max-w-[80%] flex-col items-center gap-0`}>
				<Icon style={tw`mt-5`} name="CloudSync" size={48} />
				<Text style={tw`my-2 text-center leading-5 text-ink-dull`}>
					Would you like to access cloud services to upload your library to the cloud?
				</Text>
				<Button
					style={tw`mt-3`}
					onPress={() => {
						navigation.navigate('CloudSettings');
						modalRef.current?.dismiss();
					}}
					variant="accent"
				>
					<Text style={tw`font-medium text-ink`}>Start</Text>
				</Button>
			</View>
		</Modal>
	);
});

export default CloudModal;
