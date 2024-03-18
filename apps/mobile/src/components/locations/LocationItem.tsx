import { Pressable } from 'react-native';
import { Location } from '@sd/client';
import { tw } from '~/lib/tailwind';

import { ModalRef } from '../layout/Modal';
import { LocationModal } from '../modal/location/LocationModal';
import GridLocation from './GridLocation';
import ListLocation from './ListLocation';

type LocationItemProps = {
	location: Location;
	onPress: () => void;
	viewStyle?: 'grid' | 'list';
	modalRef: React.RefObject<ModalRef>;
	editLocation: () => void;
};

export const LocationItem = ({
	location,
	onPress,
	modalRef,
	editLocation,
	viewStyle = 'grid'
}: LocationItemProps) => {
	return (
		<Pressable style={tw`flex-1`} onPress={onPress}>
			{viewStyle === 'grid' ? (
				<GridLocation onPress={onPress} location={location} modalRef={modalRef} />
			) : (
				<ListLocation onPress={onPress} location={location} modalRef={modalRef} />
			)}
			<LocationModal
				editLocation={() => {
					editLocation();
					modalRef.current?.close();
				}}
				locationId={location.id}
				ref={modalRef}
			/>
		</Pressable>
	);
};
