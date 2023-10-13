import { PropsWithChildren, ReactNode } from 'react';
import { auth } from '@sd/client';

export function AuthCheck({ fallback, children }: PropsWithChildren<{ fallback?: ReactNode }>) {
	const authState = auth.useStateSnapshot();

	if (authState.status !== 'loggedIn') return <>{fallback}</>;

	return <>{children}</>;
}
