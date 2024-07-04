import { useRef } from 'react';
import { Pressable } from 'react-native';
import { ClassInput } from 'twrnc';
import { Location } from '@sd/client';
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
	style?: ClassInput;
};

export const LocationItem = ({
	location,
	onPress,
	editLocation,
	viewStyle = 'grid',
	style
}: LocationItemProps) => {
	const modalRef = useRef<ModalRef>(null);
	return (
		<>
			<Pressable
				style={twStyle(viewStyle === 'grid' ? `m-1 w-[112px]` : `flex-1`, style)}
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
		</>
	);
};
