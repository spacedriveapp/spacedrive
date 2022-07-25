import { MotiView } from 'moti';
import React from 'react';

import Layout from '../../constants/Layout';

// Anything wrapped with FadeIn will fade in on mount.
export const FadeInAnimation = ({ children, delay }: { children: any; delay?: number }) => (
	<MotiView from={{ opacity: 0 }} animate={{ opacity: 1 }} transition={{ type: 'timing', delay }}>
		{children}
	</MotiView>
);
export const FadeInUpAnimation = ({ children, delay }: { children: any; delay?: number }) => (
	<MotiView
		from={{ opacity: 0, translateY: 20 }}
		animate={{ opacity: 1, translateY: 0 }}
		transition={{ type: 'timing', delay }}
	>
		{children}
	</MotiView>
);

export const LogoAnimation = ({ children }: { children: any }) => (
	<MotiView
		from={{ opacity: 0.8, translateY: Layout.window.width / 2 }}
		animate={{ opacity: 1, translateY: 0 }}
		transition={{ type: 'timing', delay: 200 }}
	>
		{children}
	</MotiView>
);
