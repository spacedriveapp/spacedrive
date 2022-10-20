import { PropsWithChildren } from 'react';

export const SettingsContainer = ({ children }: PropsWithChildren) => (
	<div className="flex flex-col flex-grow w-full max-w-4xl space-y-6">{children}</div>
);
