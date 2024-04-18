import LocationsScreen from '~/screens/browse/Locations';
import { ScrollY } from '~/types/shared';

const LocationSettingsScreen = ({ scrollY }: ScrollY) => {
	return <LocationsScreen scrollY={scrollY} viewStyle="list" />;
};

export default LocationSettingsScreen;
