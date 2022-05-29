import React from 'react';

interface SettingsContainerProps {
	children: React.ReactNode;
}

export const SettingsContainer: React.FC<SettingsContainerProps> = (props) => {
	return <div className="flex flex-col flex-grow max-w-4xl space-y-4 w-ful">{props.children}</div>;
};
