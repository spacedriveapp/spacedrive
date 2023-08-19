import clsx from 'clsx';
import { CheckCircle, Icon, Info, WarningCircle, X } from 'phosphor-react';
import { PropsWithChildren, ReactNode } from 'react';
import { toast as SonnerToast, ToastT } from 'sonner';
import { Button } from './Button';

type ToastId = ToastT['id'];
type ToastType = 'info' | 'success' | 'error';
type ToastMessage = ReactNode | { title: string; description?: string };

interface ToastAction {
	label: string;
	onClick: () => void;
	className?: string;
}

interface ToastProps extends PropsWithChildren {
	id: ToastId;
	type?: ToastType;
	message: ToastMessage;
	icon?: ReactNode;
	action?: ToastAction;
	cancel?: Partial<Pick<ToastAction, 'onClick'>> & Omit<ToastAction, 'onClick'>;
}

const icons: Record<ToastType, Icon> = {
	success: CheckCircle,
	error: WarningCircle,
	info: Info
};

const Toast = ({ id, type, message, icon, action, cancel }: ToastProps) => {
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
				'relative flex w-full gap-2',
				description ? 'items-start' : 'items-center'
			)}
		>
			{(icon || type) && (
				<div className={clsx(description && 'mt-px')}>
					{icon || (type && typeIcon(type))}
				</div>
			)}

			<div className="flex grow flex-col gap-0.5 text-sm">
				{title && <span className="font-medium text-ink">{title}</span>}

				{description && <span>{description}</span>}

				{(action || cancel) && (
					<div className="mt-2 flex gap-2">
						{action && (
							<Button
								variant="accent"
								onClick={() => {
									action.onClick();
									SonnerToast.dismiss(id);
								}}
								className={clsx(
									'!rounded !px-1.5 !py-0.5 !font-normal',
									action.className
								)}
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
								className={clsx(
									'!rounded !px-1.5 !py-0.5 !font-normal',
									cancel.className
								)}
							>
								{cancel.label}
							</Button>
						)}
					</div>
				)}
			</div>

			<button
				className="relative text-sm transition-colors before:absolute before:-inset-2 before:content-[''] hover:text-ink"
				onClick={() => SonnerToast.dismiss(id)}
			>
				<X weight="bold" />
			</button>
		</div>
	);
};

interface Options
	extends Omit<
		Partial<ToastT>,
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
	type?: ToastType;
	action?: ToastAction;
	cancel?: Partial<Pick<ToastAction, 'onClick'>> & Omit<ToastAction, 'onClick'>;
}

const renderToast = (
	message: ToastMessage,
	{ className, type, icon, action, cancel, ...options }: Options = {}
) =>
	SonnerToast.custom(
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
			className: clsx(
				'w-full cursor-default select-none overflow-hidden rounded-md border border-app-line bg-app-darkBox/80 p-3 text-ink-faint backdrop-blur',
				className
			),
			...options
		}
	);

export const toast = Object.assign(renderToast, {
	info: (message: ToastMessage, options?: Options) => {
		return renderToast(message, { ...options, type: 'info' });
	},
	success: (message: ToastMessage, options?: Options) => {
		return renderToast(message, { ...options, type: 'success' });
	},
	error: (message: ToastMessage, options?: Options) => {
		return renderToast(message, { ...options, type: 'error' });
	}
});

export { Toaster } from 'sonner';
