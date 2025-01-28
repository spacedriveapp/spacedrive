'use client';

import { PropsWithChildren } from 'react';
import { ParallaxProvider } from 'react-scroll-parallax';
import { TooltipProvider } from '@sd/ui';

export function ClientProviders({ children }: PropsWithChildren) {
	return (
		<ParallaxProvider>
			<TooltipProvider>{children}</TooltipProvider>
		</ParallaxProvider>
	);
}
