import { Navigate, Route, Routes } from 'react-router-dom';
import { useCurrentLibrary, useInvalidateQuery } from '@sd/client';
import { AppLayout } from '~/AppLayout';
import { useKeybindHandler } from '~/hooks/useKeyboardHandler';
import screens from '~/screens';
import { lazyEl } from '~/util';
import OnboardingRoot, { ONBOARDING_SCREENS } from './components/onboarding/OnboardingRoot';

const NotFound = lazyEl(() => import('./NotFound'));

export function AppRouter() {
	const { library } = useCurrentLibrary();

	useKeybindHandler();
	useInvalidateQuery();

	return (
		<Routes>
			<Route path="onboarding" element={<OnboardingRoot />}>
				<Route index element={<Navigate to="start" />} />
				{ONBOARDING_SCREENS.map(({ key, component: ScreenComponent }, index) => (
					<Route key={key} path={key} element={<ScreenComponent />} />
				))}
			</Route>

			<Route element={<AppLayout />}>
				{/* As we are caching the libraries in localStore so this *shouldn't* result is visual problems unless something else is wrong */}
				{library === undefined ? (
					<Route
						path="*"
						element={
							<h1 className="p-4 text-white">Please select or create a library in the sidebar.</h1>
						}
					/>
				) : (
					<>
						{screens}
						<Route path="*" element={NotFound} />
					</>
				)}
			</Route>
		</Routes>
	);
}
