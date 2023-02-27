import { Navigate, useRoutes } from 'react-router-dom';
import { currentLibraryCache, useCachedLibraries, useInvalidateQuery } from '@sd/client';
import AppLayout from '~/AppLayout';
import { useKeybindHandler } from '~/hooks/useKeyboardHandler';
import screens from '~/screens';
import OnboardingRoot, { ONBOARDING_ROUTES } from './components/onboarding/OnboardingRoot';

function Index() {
	const libraries = useCachedLibraries();

	if (libraries.status !== 'success') return null;

	if (libraries.data.length === 0) return <Navigate to="onboarding" />;

	const currentLibrary = libraries.data.find((l) => l.uuid === currentLibraryCache.id);

	const libraryId = currentLibrary ? currentLibrary.uuid : libraries.data[0]?.uuid;

	return <Navigate to={`${libraryId}/overview`} />;
}

export function AppRouter() {
	useKeybindHandler();
	useInvalidateQuery();

	return useRoutes([
		{
			index: true,
			element: <Index />
		},
		{
			path: 'onboarding',
			element: <OnboardingRoot />,
			children: ONBOARDING_ROUTES
		},
		{
			path: ':libraryId',
			element: <AppLayout />,
			children: screens
		}
	]);
}
