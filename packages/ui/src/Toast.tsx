'use client';

import { CheckCircle, Icon, Info, Warning, WarningCircle, X } from '@phosphor-icons/react';
import clsx from 'clsx';
import { CSSProperties, ForwardedRef, forwardRef, ReactNode, useEffect, useState } from 'react';
import { toast as SonnerToast } from 'sonner';

import { Button } from './Button';
import { Loader } from './Loader';

export const TOAST_TIMEOUT = 4000;

const actionButtonClassName = '!rounded !px-1.5 !py-0.5 !font-normal';

const toastClassName = clsx(
	'w-full overflow-hidden rounded-md p-3 shadow-lg',
	'cursor-default select-none',
	'border border-app-line',
	'bg-app-darkBox/90 backdrop-blur',
	'text-sm text-ink-faint'
);

export type ToastId<T = string | number> = T;
type ToastType = 'info' | 'success' | 'error' | 'warning';
export type ToastMessage = ReactNode | { title: ReactNode; body?: ReactNode };
type ToastPromiseData = unknown;
type ToastPromise<T = ToastPromiseData> = Promise<T> | (() => Promise<T>);
type ToastAction = { label: string; onClick: () => void; className?: string };
type ToastCloseEvent = 'on-dismiss' | 'on-auto-close' | 'on-action' | 'on-cancel';

interface ToastOptions {
	id?: ToastId;
	ref?: React.Ref<HTMLDivElement>;
	type?: ToastType;
	icon?: ReactNode;
	duration?: number;
	action?: ToastAction;
	cancel?: (Omit<ToastAction, 'onClick'> & { onClick?: ToastAction['onClick'] }) | string;
	onClose?: (data: { id: ToastId; event: ToastCloseEvent }) => void;
	onDismiss?: (id: ToastId) => void;
	onAutoClose?: (id: ToastId) => void;
	important?: boolean;
	style?: CSSProperties;
	className?: string;
}

interface PromiseToastOptions<T = ToastPromiseData>
	extends Omit<ToastOptions, 'ref' | 'icon' | 'type' | 'action' | 'cancel' | 'onClose'> {
	loading: ToastMessage;
	success: ToastMessage | ((data: T) => ToastMessage);
	error: ToastMessage | ((error: unknown) => ToastMessage);
	loader?: ReactNode;
	showLoader?: boolean;
	onClose?: (data: {
		id: ToastId;
		event: Extract<ToastCloseEvent, 'on-dismiss' | 'on-auto-close'>;
	}) => void;
}

type CustomToastOptions = Omit<
	ToastOptions,
	'type' | 'icon' | 'action' | 'cancel' | 'onDismiss' | 'onClose'
>;

interface ToastProps
	extends Pick<ToastOptions, 'type' | 'icon' | 'action' | 'cancel' | 'onDismiss' | 'onClose'> {
	id: ToastId;
	message: ToastMessage | ((id: ToastId) => ToastMessage);
	closable?: boolean;
}

const icons: Record<ToastType, Icon> = {
	success: CheckCircle,
	error: WarningCircle,
	info: Info,
	warning: Warning
};

const Toast = forwardRef<HTMLDivElement, ToastProps>(
	({ closable = true, action, cancel, ...props }, ref) => {
		const message =
			typeof props.message === 'function' ? props.message(props.id) : props.message;

		const title =
			message && typeof message === 'object' && 'title' in message ? message.title : message;

		const body =
			message && typeof message === 'object' && 'body' in message ? message.body : undefined;

		const typeIcon = (type: ToastType) => {
			const Icon = icons[type];
			return (
				<Icon
					size={16}
					weight="fill"
					className={clsx(
						type === 'success' && 'text-green-500',
						type === 'error' && 'text-red-500',
						type === 'warning' && 'text-yellow-500'
					)}
				/>
			);
		};

		return (
			<div
				ref={ref}
				className={clsx(
					'pointer-events-auto flex gap-2',
					body || action || cancel ? 'items-start' : 'items-center'
				)}
			>
				{(props.icon || props.type) && (
					<div className={clsx((body || action || cancel) && 'mt-px')}>
						{props.icon || (props.type && typeIcon(props.type))}
					</div>
				)}

				<div className="flex grow flex-col">
					{title && (
						<span className="font-medium text-ink" style={{ wordBreak: 'break-word' }}>
							{title}
						</span>
					)}

					{body && (
						<div className="mt-0.5" style={{ wordBreak: 'break-word' }}>
							{body}
						</div>
					)}

					{(action || cancel) && (
						<div className="mt-2.5 flex gap-2">
							{action && (
								<Button
									variant="accent"
									onClick={() => {
										action.onClick();
										props.onClose?.({ id: props.id, event: 'on-action' });
										toast.dismiss(props.id);
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
										if (typeof cancel === 'object') cancel.onClick?.();
										props.onClose?.({ id: props.id, event: 'on-cancel' });
										toast.dismiss(props.id);
									}}
									className={clsx(
										actionButtonClassName,
										typeof cancel === 'object' ? cancel.className : null
									)}
								>
									{typeof cancel === 'object' ? cancel.label : cancel}
								</Button>
							)}
						</div>
					)}
				</div>

				{closable && (
					<button
						className="relative transition-colors before:absolute before:-inset-2 before:content-[''] hover:text-ink"
						onClick={() => {
							props.onDismiss?.(props.id);
							props.onClose?.({ id: props.id, event: 'on-dismiss' });
							toast.dismiss(props.id);
						}}
					>
						<X weight="bold" />
					</button>
				)}
			</div>
		);
	}
);

interface PromiseToastProps<T = ToastPromiseData>
	extends Pick<
		PromiseToastOptions<T>,
		| 'loading'
		| 'success'
		| 'error'
		| 'onDismiss'
		| 'onAutoClose'
		| 'onClose'
		| 'loader'
		| 'showLoader'
	> {
	id: ToastId;
	promise: ToastPromise<T>;
	duration?: number;
}

const PromiseToast = <T extends ToastPromiseData>({
	showLoader = true,
	...props
}: PromiseToastProps<T>) => {
	const [type, setType] = useState<ToastType>();
	const [message, setMessage] = useState<ToastMessage>(props.loading);

	useEffect(() => {
		const resolve = async () => {
			try {
				const res = await (props.promise instanceof Promise
					? props.promise
					: props.promise());
				const message =
					typeof props.success === 'function' ? props.success(res) : props.success;
				setMessage(message);
				setType('success');
			} catch (err) {
				const message = typeof props.error === 'function' ? props.error(err) : props.error;
				setMessage(message);
				setType('error');
			}

			setTimeout(() => {
				props.onAutoClose?.(props.id);
				props.onClose?.({ id: props.id, event: 'on-auto-close' });
				toast.dismiss(props.id);
			}, props.duration || TOAST_TIMEOUT);
		};

		resolve();
	}, [props.id, props.promise, props.success, props.error, props.duration, props]);

	return (
		<Toast
			id={props.id}
			type={type}
			message={message}
			icon={!type && showLoader && (props.loader ?? <Loader className="!h-4 !w-4" />)}
			closable={!!type}
			onDismiss={props.onDismiss}
			onClose={({ id, event }) => {
				if (event === 'on-action' || event === 'on-cancel') return;
				props.onClose?.({ id, event });
			}}
		/>
	);
};

const renderToast = (
	message: ToastMessage | ((id: ToastId) => ToastMessage),
	{
		ref,
		type,
		icon,
		action,
		cancel,
		onDismiss,
		onClose,
		onAutoClose,
		className,
		...options
	}: ToastOptions = {}
) => {
	return SonnerToast.custom(
		(id) => (
			<Toast
				id={id}
				ref={ref}
				type={type}
				icon={icon}
				message={message}
				action={action}
				cancel={cancel}
				onDismiss={onDismiss}
				onClose={onClose}
			/>
		),
		{
			className: clsx(toastClassName, className),
			onAutoClose: ({ id }) => {
				onAutoClose?.(id);
				onClose?.({ id, event: 'on-auto-close' });
			},
			...options
		}
	);
};

const renderCustomToast = (
	jsx: Parameters<typeof SonnerToast.custom>[0],
	{ onAutoClose, className, ...options }: CustomToastOptions = {}
) => {
	return SonnerToast.custom(jsx, {
		className: clsx(toastClassName, className),
		onAutoClose: ({ id }) => onAutoClose?.(id),
		...options
	});
};

const renderPromiseToast = <T extends ToastPromiseData>(
	promise: ToastPromise<T>,
	{
		loading,
		success,
		error,
		onDismiss,
		onAutoClose,
		onClose,
		duration,
		className,
		loader,
		showLoader,
		...options
	}: PromiseToastOptions<T>
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
				onDismiss={onDismiss}
				onAutoClose={onAutoClose}
				onClose={onClose}
				loader={loader}
				showLoader={showLoader}
			/>
		),
		{
			duration: Infinity,
			className: clsx(toastClassName, className),
			...options
		}
	);
};

export const toast = Object.assign(renderToast, {
	info: (
		message: ToastMessage | ((id: ToastId) => ToastMessage),
		options?: Omit<ToastOptions, 'type'>
	) => {
		return renderToast(message, { ...options, type: 'info' });
	},
	success: (
		message: ToastMessage | ((id: ToastId) => ToastMessage),
		options?: Omit<ToastOptions, 'type'>
	) => {
		return renderToast(message, { ...options, type: 'success' });
	},
	error: (
		message: ToastMessage | ((id: ToastId) => ToastMessage),
		options?: Omit<ToastOptions, 'type'>
	) => {
		return renderToast(message, { ...options, type: 'error' });
	},
	warning: (
		message: ToastMessage | ((id: ToastId) => ToastMessage),
		options?: Omit<ToastOptions, 'type'>
	) => {
		return renderToast(message, { ...options, type: 'warning' });
	},
	custom: renderCustomToast,
	promise: renderPromiseToast,
	dismiss: SonnerToast.dismiss
});

export { Toaster } from 'sonner';
