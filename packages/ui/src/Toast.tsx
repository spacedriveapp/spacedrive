import clsx from 'clsx';
import { CheckCircle, Icon, Info, WarningCircle, X } from 'phosphor-react';
import { ReactNode, useEffect, useState } from 'react';
import { toast as SonnerToast, ToastT } from 'sonner';
import { Button } from './Button';
import { Loader } from './Loader';

type ToastId = ToastT['id'];
type ToastType = 'info' | 'success' | 'error';
type ToastMessage = ReactNode | { title: string; description?: string };
type ToastPromiseData = unknown;
type ToastPromise<T = ToastPromiseData> = Promise<T> | (() => Promise<T>);
type ToastAction = { label: string; onClick: () => void; className?: string };

interface ToastOptions
	extends Omit<
		ToastT,
		| 'id'
		| 'type'
		| 'action'
		| 'cancel'
		| 'delete'
		| 'promise'
		| 'jsx'
		| 'title'
		| 'description'
		| 'descriptionClassName'
	> {
	id?: ToastId;
	type?: ToastType;
	action?: ToastAction;
	cancel?: Omit<ToastAction, 'onClick'> & { onClick?: ToastAction['onClick'] };
}

interface ToastPromiseOptions<T = ToastPromiseData> {
	loading: ReactNode;
	success: ReactNode | ((data: T) => ReactNode);
	error: ReactNode | ((error: unknown) => ReactNode);
}

const toastClassName =
	'w-full cursor-default select-none overflow-hidden rounded-md border border-app-line bg-app-darkBox/90 shadow-lg p-3 text-sm text-ink-faint backdrop-blur';

const actionButtonClassName = '!rounded !px-1.5 !py-0.5 !font-normal';

interface ToastProps {
	id: ToastId;
	type?: ToastType;
	message: ToastMessage;
	icon?: ReactNode;
	action?: ToastAction;
	cancel?: ToastOptions['cancel'];
	closable?: boolean;
}

const icons: Record<ToastType, Icon> = {
	success: CheckCircle,
	error: WarningCircle,
	info: Info
};

const Toast = ({ id, type, message, icon, action, cancel, closable = true }: ToastProps) => {
	const title =
		message && typeof message === 'object' && 'title' in message ? message.title : message;

	const description =
		message && typeof message === 'object' && 'description' in message
			? message.description
			: undefined;

	const typeIcon = (type: ToastType) => {
		const Icon = icons[type];
		return (
			<Icon
				size={16}
				weight="fill"
				className={clsx(
					type === 'success' && 'text-green-500',
					type === 'error' && 'text-red-500'
				)}
			/>
		);
	};

	return (
		<div
			className={clsx(
				'flex gap-2',
				description || action || cancel ? 'items-start' : 'items-center'
			)}
		>
			{(icon || type) && (
				<div className={clsx((description || action || cancel) && 'mt-px')}>
					{icon || (type && typeIcon(type))}
				</div>
			)}

			<div className="flex grow flex-col">
				{title && <span className="font-medium text-ink">{title}</span>}

				{description && <span className="mt-0.5">{description}</span>}

				{(action || cancel) && (
					<div className="mt-2.5 flex gap-2">
						{action && (
							<Button
								variant="accent"
								onClick={() => {
									action.onClick();
									SonnerToast.dismiss(id);
								}}
								className={clsx(actionButtonClassName, action.className)}
							>
								{action.label}
							</Button>
						)}

						{cancel && (
							<Button
								variant="gray"
								onClick={() => {
									cancel.onClick?.();
									SonnerToast.dismiss(id);
								}}
								className={clsx(actionButtonClassName, cancel.className)}
							>
								{cancel.label}
							</Button>
						)}
					</div>
				)}
			</div>

			{closable && (
				<button
					className="relative transition-colors before:absolute before:-inset-2 before:content-[''] hover:text-ink"
					onClick={() => toast.dismiss(id)}
				>
					<X weight="bold" />
				</button>
			)}
		</div>
	);
};

interface PromiseToastProps<T = ToastPromiseData> extends ToastPromiseOptions<T> {
	id: ToastId;
	promise: ToastPromise<T>;
	duration?: number;
}

const PromiseToast = <T extends ToastPromiseData>({
	id,
	promise,
	loading,
	success,
	error,
	duration
}: PromiseToastProps<T>) => {
	const [type, setType] = useState<ToastType>();
	const [message, setMessage] = useState<ToastMessage>(loading);

	useEffect(() => {
		const resolve = async () => {
			try {
				const res = await (promise instanceof Promise ? promise : promise());
				const message = typeof success === 'function' ? success(res) : success;
				setMessage(message);
				setType('success');
			} catch (err) {
				const message = typeof error === 'function' ? error(err) : error;
				setMessage(message);
				setType('error');
			}

			setTimeout(() => toast.dismiss(id), duration || 4000);
		};

		resolve();
	}, [id, promise, success, error, duration]);

	return (
		<Toast
			id={id}
			type={type}
			message={message}
			icon={!type && <Loader className="!h-4 !w-4" />}
			closable={!!type}
		/>
	);
};

const renderToast = (
	message: ToastMessage,
	{ className, type, icon, action, cancel, ...options }: ToastOptions = {}
) => {
	return SonnerToast.custom(
		(id) => (
			<Toast
				id={id}
				message={message}
				type={type}
				icon={icon}
				action={action}
				cancel={cancel}
			/>
		),
		{
			className: clsx(toastClassName, className),
			...options
		}
	);
};

const renderCustomToast = (
	jsx: Parameters<typeof SonnerToast.custom>[0],
	{ className, ...options }: Omit<ToastOptions, 'icon' | 'type' | 'action' | 'cancel'> = {}
) => {
	return SonnerToast.custom(jsx, {
		className: clsx(toastClassName, className),
		...options
	});
};

const renderPromiseToast = <T extends ToastPromiseData>(
	promise: ToastPromise<T>,
	{
		className,
		loading,
		success,
		error,
		duration,
		...options
	}: Omit<ToastOptions, 'icon' | 'type' | 'action' | 'cancel'> & ToastPromiseOptions<T>
) => {
	return SonnerToast.custom(
		(id) => (
			<PromiseToast
				id={id}
				promise={promise}
				loading={loading}
				success={success}
				error={error}
				duration={duration}
			/>
		),
		{
			className: clsx(toastClassName, className),
			duration: Infinity,
			...options
		}
	);
};

export const toast = Object.assign(renderToast, {
	info: (message: ToastMessage, options?: Omit<ToastOptions, 'type'>) => {
		return renderToast(message, { ...options, type: 'info' });
	},
	success: (message: ToastMessage, options?: Omit<ToastOptions, 'type'>) => {
		return renderToast(message, { ...options, type: 'success' });
	},
	error: (message: ToastMessage, options?: Omit<ToastOptions, 'type'>) => {
		return renderToast(message, { ...options, type: 'error' });
	},
	custom: renderCustomToast,
	promise: renderPromiseToast,
	dismiss: SonnerToast.dismiss
});

export { Toaster } from 'sonner';
