import { ReactNode } from 'react';

export interface SettingsSubHeaderProps {
	title: string;
	rightArea?: ReactNode;
}

export const SettingsSubHeader: React.FC<SettingsSubHeaderProps> = (props) => {
	return (
		<div className="flex">
			<div className="flex-grow">
				<h1 className="text-xl font-bold">{props.title}</h1>
			</div>
			{props.rightArea}
		</div>
	);
};
