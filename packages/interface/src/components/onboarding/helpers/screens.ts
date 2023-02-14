import { useLocation } from 'react-router-dom';

export const ONBOARDING_ROUTE_PREFIX_NAME = 'onboarding';

export const useCurrentOnboardingScreenKey = (): string | null => {
	const { pathname } = useLocation();

	if (pathname.startsWith(`/${ONBOARDING_ROUTE_PREFIX_NAME}/`)) {
		return pathname.split('/')[2];
	}

	return null;
};
