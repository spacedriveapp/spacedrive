import { Route, Routes } from 'react-router-dom';
import { useCurrentLibrary, useInvalidateQuery } from '@sd/client';
import { useKeybindHandler } from '~/hooks/useKeyboardHandler';
import screens from '~/screens';
import { lazyEl } from '~/util';
import { AppLayout } from './AppLayout';

const Onboarding = lazyEl(() => import('./components/onboarding/Onboarding'));
const NotFound = lazyEl(() => import('./NotFound'));

export function AppRouter() {
	const { library } = useCurrentLibrary();

	useKeybindHandler();
	useInvalidateQuery();

	return (
		<Routes>
			<Route path="onboarding" element={Onboarding} />
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
