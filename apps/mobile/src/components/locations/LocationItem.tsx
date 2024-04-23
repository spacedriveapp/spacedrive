import { Location } from '@sd/client';
import { useRef } from 'react';
import { Pressable } from 'react-native';
import { twStyle } from '~/lib/tailwind';

import { ModalRef } from '../layout/Modal';
import { LocationModal } from '../modal/location/LocationModal';
import GridLocation from './GridLocation';
import ListLocation from './ListLocation';

type LocationItemProps = {
	location: Location;
	onPress: () => void;
	viewStyle?: 'grid' | 'list';
	editLocation: () => void;
};

export const LocationItem = ({
	location,
	onPress,
	editLocation,
	viewStyle = 'grid'
}: LocationItemProps) => {
	const modalRef = useRef<ModalRef>(null);
	return (
			<Pressable
				style={twStyle(viewStyle === 'grid' ? `w-[31.5%]` : `flex-1`)}
				onPress={onPress}
			>
				{viewStyle === 'grid' ? (
					<>
						<GridLocation location={location} modalRef={modalRef} />
						<LocationModal
							editLocation={() => {
								editLocation();
								modalRef.current?.close();
							}}
							locationId={location.id}
							ref={modalRef}
						/>
					</>
				) : (
					<ListLocation location={location} />
				)}
			</Pressable>
	);
};
