import * as ToastPrimitive from '@radix-ui/react-toast';
import clsx from 'clsx';
import { useToasts } from '../../hooks/useToasts';

export function Toasts() {
	const { toasts, addToast, removeToast } = useToasts();
	return (
		<div className="fixed right-0 flex">
			<ToastPrimitive.Provider>
				<>
					{toasts.map((toast) => (
						<ToastPrimitive.Root
							key={toast.id}
							open={true}
							onOpenChange={() => removeToast(toast)}
							duration={toast.duration || 3000}
							className={clsx(
								'w-80 m-4 shadow-lg rounded-lg',
								'bg-app-box/20 backdrop-blur',
								'radix-state-open:animate-toast-slide-in-bottom md:radix-state-open:animate-toast-slide-in-right',
								'radix-state-closed:animate-toast-hide',
								'radix-swipe-end:animate-toast-swipe-out',
								'translate-x-radix-toast-swipe-move-x',
								'radix-swipe-cancel:translate-x-0 radix-swipe-cancel:duration-200 radix-swipe-cancel:ease-[ease]',
								'focus:outline-none focus-visible:ring focus-visible:ring-accent focus-visible:ring-opacity-75 border-white/10 border-2 shadow-2xl'
							)}
						>
							<div className="flex">
								<div className="flex items-center flex-1 w-0 py-4 pl-5">
									<div className="w-full radix">
										<ToastPrimitive.Title className="text-sm font-medium text-black">
											{toast.title}
										</ToastPrimitive.Title>
										{toast.subtitle && (
											<ToastPrimitive.Description className="mt-1 text-sm text-black">
												{toast.subtitle}
											</ToastPrimitive.Description>
										)}
									</div>
								</div>
								<div className="flex">
									<div className="flex flex-col px-3 py-2 space-y-1">
										<div className="flex flex-1 h-0">
											{toast.actionButton && (
												<ToastPrimitive.Action
													altText="view now"
													className="flex items-center justify-center w-full px-3 py-2 text-sm font-medium border border-transparent rounded-lg text-accent hover:bg-white/10 focus:z-10 focus:outline-none focus-visible:ring focus-visible:ring-accent focus-visible:ring-opacity-75"
													onClick={(e) => {
														e.preventDefault();
														toast.actionButton?.onClick();
														removeToast(toast);
													}}
												>
													{toast.actionButton.text || 'Open'}
												</ToastPrimitive.Action>
											)}
										</div>
										<div className="flex flex-1 h-0">
											<ToastPrimitive.Close className="flex items-center justify-center w-full px-3 py-2 text-sm font-medium border border-transparent rounded-lg text-ink-faint hover:bg-white/10 focus:z-10 focus:outline-none focus-visible:ring focus-visible:ring-accent focus-visible:ring-opacity-75">
												Dismiss
											</ToastPrimitive.Close>
										</div>
									</div>
								</div>
							</div>
						</ToastPrimitive.Root>
					))}

					<ToastPrimitive.Viewport />
				</>
			</ToastPrimitive.Provider>
		</div>
	);
}
