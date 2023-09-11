import clsx from 'clsx';

export default ({ component: Icon, className, ...props }: any) => (
	<Icon weight="bold" className={clsx('mr-2 h-4 w-4', className)} {...props} />
);
