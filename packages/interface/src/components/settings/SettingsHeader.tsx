import clsx from 'clsx';
import { ReactNode } from 'react';

interface SettingsHeaderProps {
	title: string;
	description: string;
	rightArea?: ReactNode;
}

export const SettingsHeader: React.FC<SettingsHeaderProps> = (props) => {
	return (
		<div className="flex mt-3 mb-3">
			<div className="flex-grow">
				<h1 className="text-2xl font-bold">{props.title}</h1>
				<p className="mt-1 text-sm text-gray-400">{props.description}</p>
			</div>
			{props.rightArea}
			<hr className="mt-4 border-gray-550" />
		</div>
	);
};

export const SettingsIcon = ({ component: Icon, ...props }: any) => (
	<Icon weight="bold" {...props} className={clsx('w-4 h-4 mr-2', props.className)} />
);

export const SettingsHeading: React.FC<{ className?: string; children: string }> = ({
	children,
	className
}) => (
	<div className={clsx('mt-5 mb-1 ml-1 text-xs font-semibold text-gray-400', className)}>
		{children}
	</div>
);
