export function shouldInstallMiddleClickNavigationGuard(
	platform: 'web' | 'tauri',
	userAgent: string
): boolean {
	return platform === 'tauri' && userAgent.includes('Windows');
}

export function installMiddleClickNavigationGuard(
	target: EventTarget
): () => void {
	const preventMiddleClickNavigation = (event: Event) => {
		const mouseEvent = event as MouseEvent;

		if (mouseEvent.button === 1) mouseEvent.preventDefault();
	};

	target.addEventListener('auxclick', preventMiddleClickNavigation, {
		passive: false
	});

	return () =>
		target.removeEventListener('auxclick', preventMiddleClickNavigation);
}

export function installMiddleClickNavigationGuardForPlatform(
	platform: 'web' | 'tauri',
	environment?: {userAgent: string; eventTarget: EventTarget}
): (() => void) | undefined {
	if (
		!environment ||
		!shouldInstallMiddleClickNavigationGuard(
			platform,
			environment.userAgent
		)
	) {
		return;
	}

	return installMiddleClickNavigationGuard(environment.eventTarget);
}
