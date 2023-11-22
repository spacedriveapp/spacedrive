import { Navigate, RouteObject } from 'react-router';
import { getOnboardingStore } from '@sd/client';
import { OperatingSystem } from '~/util/Platform';

import Alpha from './alpha';
import { useOnboardingContext } from './context';
import CreatingLibrary from './creating-library';
import Locations from './locations';
import NewLibrary from './new-library';
import Privacy from './privacy';

const Index = () => {
	const obStore = getOnboardingStore();
	const ctx = useOnboardingContext();

	if (obStore.lastActiveScreen && !ctx.library)
		return <Navigate to={obStore.lastActiveScreen} replace />;

	return <Navigate to="alpha" replace />;
};

const onboardingRoutes = (os: OperatingSystem) => {
	return [
		{ index: true, element: <Index /> },
		{ path: 'alpha', element: <Alpha /> },
		{ path: 'new-library', element: <NewLibrary /> },
		{ path: 'locations', element: <Locations /> },
		{ path: 'privacy', element: <Privacy /> },
		{ path: 'creating-library', element: <CreatingLibrary /> }
	] satisfies RouteObject[];
};

export default onboardingRoutes;
