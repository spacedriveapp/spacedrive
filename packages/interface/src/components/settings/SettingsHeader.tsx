import React from 'react';

interface SettingsHeaderProps {
	title: string;
	description: string;
}

export const SettingsHeader: React.FC<SettingsHeaderProps> = (props) => {
	return (
		<div className="mt-3 mb-3">
			<h1 className="text-2xl font-bold">{props.title}</h1>
			<p className="mt-1 text-sm text-gray-400">{props.description}</p>
			<hr className="mt-4 border-gray-550" />
		</div>
	);
};
