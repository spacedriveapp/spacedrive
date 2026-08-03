export type MiddleClickNavigationEvent = {
	button: number;
	preventDefault(): void;
};

export type MiddleClickNavigationGuardTarget = {
	addEventListener(
		type: 'auxclick',
		listener: (event: MiddleClickNavigationEvent) => void,
		options: {passive: false}
	): void;
	removeEventListener(
		type: 'auxclick',
		listener: (event: MiddleClickNavigationEvent) => void
	): void;
};

export function shouldInstallMiddleClickNavigationGuard(
	platform: 'web' | 'tauri',
	userAgent: string
): boolean {
	return platform === 'tauri' && userAgent.includes('Windows');
}

export function installMiddleClickNavigationGuard(
	target: MiddleClickNavigationGuardTarget
): () => void {
	const preventMiddleClickNavigation = (event: MiddleClickNavigationEvent) => {
		if (event.button === 1) event.preventDefault();
	};

	target.addEventListener('auxclick', preventMiddleClickNavigation, {
		passive: false
	});

	return () =>
		target.removeEventListener('auxclick', preventMiddleClickNavigation);
}

export function installMiddleClickNavigationGuardForPlatform(
	platform: 'web' | 'tauri',
	environment?: {
		userAgent: string;
		eventTarget: MiddleClickNavigationGuardTarget;
	}
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
