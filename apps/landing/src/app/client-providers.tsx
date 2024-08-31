'use client';

import { PropsWithChildren } from 'react';
import { TooltipProvider } from '@sd/ui';

export function ClientProviders({ children }: PropsWithChildren) {
	return <TooltipProvider>{children}</TooltipProvider>;
}
