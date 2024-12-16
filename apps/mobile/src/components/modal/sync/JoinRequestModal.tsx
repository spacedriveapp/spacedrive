import { ArrowRight } from 'phosphor-react-native';
import React, { forwardRef } from 'react';
import { Text, View } from 'react-native';
import { HardwareModel } from '@sd/client';
import { Icon } from '~/components/icons/Icon';
import { Modal, ModalRef } from '~/components/layout/Modal';
import { hardwareModelToIcon } from '~/components/overview/Devices';
import { Button } from '~/components/primitive/Button';
import useForwardedRef from '~/hooks/useForwardedRef';
import { twStyle } from '~/lib/tailwind';

interface Props {
	device_name: string;
	device_model: HardwareModel;
	library_name: string;
}

const JoinRequestModal = forwardRef<ModalRef, Props>((props, ref) => {
	const modalRef = useForwardedRef(ref);
	return (
		<Modal ref={modalRef} snapPoints={['36']} title="Sync request">
			<View style={twStyle('px-6')}>
				<Text style={twStyle('mx-auto mt-2 text-center text-ink-dull')}>
					A device is requesting to join one of your libraries. Please review the device
					and the library it is requesting to join below.
				</Text>
				<View style={twStyle('my-7 flex-row items-center justify-center gap-10')}>
					<View style={twStyle('flex flex-col items-center justify-center gap-2')}>
						<Icon
							// once backend endpoint is populated need to check if this is working correctly i.e fetching correct icons for devices
							name={hardwareModelToIcon(props.device_model)}
							alt="Device icon"
							size={48}
						/>
						<Text style={twStyle('text-sm font-bold text-ink')}>
							{props.device_name}
						</Text>
					</View>
					<ArrowRight weight="bold" color="#ABACBA" size={18} />
					{/* library */}
					<View style={twStyle('flex flex-col items-center justify-center gap-2')}>
						<Icon
							// once backend endpoint is populated need to check if this is working correctly i.e fetching correct icons for devices
							name={'Book'}
							alt="Device icon"
							size={48}
						/>
						<Text style={twStyle('text-sm font-bold text-ink')}>
							{props.library_name}
						</Text>
					</View>
				</View>
				<View style={twStyle('mx-auto flex-row justify-center gap-5')}>
					<Button style={twStyle('flex-1')} variant="gray">
						<Text style={twStyle('font-bold text-ink-dull')}>Cancel</Text>
					</Button>
					<Button style={twStyle('flex-1')} variant="accent">
						<Text style={twStyle('font-bold text-ink')}>Accept</Text>
					</Button>
				</View>
			</View>
		</Modal>
	);
});

export default JoinRequestModal;
