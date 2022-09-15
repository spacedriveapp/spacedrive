import clsx from 'clsx';
import { Outlet } from 'react-router';

interface SettingsScreenContainerProps {
	children: React.ReactNode;
}

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

export const SettingsScreenContainer: React.FC<SettingsScreenContainerProps> = (props) => {
	return (
		<div className="flex flex-row w-full">
			<div className="h-full border-r max-w-[200px] flex-shrink-0 border-gray-100 w-60 dark:border-gray-550">
				<div data-tauri-drag-region className="w-full h-7" />
				<div className="p-5 pt-0">{props.children}</div>
			</div>
			<div className="w-full">
				<div data-tauri-drag-region className="w-full h-7" />
				<div className="flex flex-grow-0 w-full h-full max-h-screen custom-scroll page-scroll">
					<div className="flex flex-grow px-12 pb-5">
						<Outlet />
						<div className="block h-20" />
					</div>
				</div>
			</div>
		</div>
	);
};
