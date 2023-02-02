import clsx from 'clsx';
import { PropsWithChildren, ReactNode } from 'react';

interface SettingsHeaderProps extends PropsWithChildren {
	title: string;
	description: string | ReactNode;
	rightArea?: ReactNode;
}

export const SettingsHeader: React.FC<SettingsHeaderProps> = (props) => {
	return (
		<div className="mb-3 flex">
			{props.children}
			<div className="flex-grow">
				<h1 className="text-2xl font-bold">{props.title}</h1>
				<p className="mt-1 text-sm text-gray-400">{props.description}</p>
			</div>
			{props.rightArea}
			<hr className="border-gray-550 mt-4" />
		</div>
	);
};

export const SettingsIcon = ({ component: Icon, ...props }: any) => (
	<Icon weight="bold" {...props} className={clsx('mr-2 h-4 w-4', props.className)} />
);

export function SettingsHeading({
	children,
	className
}: PropsWithChildren<{ className?: string }>) {
	return (
		<div className={clsx('mt-5 mb-1 ml-1 text-xs font-semibold text-gray-400', className)}>
			{children}
		</div>
	);
}
