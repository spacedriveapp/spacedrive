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
								'm-4 w-80 rounded-lg',
								'bg-app-box/20 backdrop-blur',
								'radix-state-open:animate-toast-slide-in-bottom md:radix-state-open:animate-toast-slide-in-right',
								'radix-state-closed:animate-toast-hide',
								'radix-swipe-end:animate-toast-swipe-out',
								'translate-x-radix-toast-swipe-move-x',
								'radix-swipe-cancel:translate-x-0 radix-swipe-cancel:duration-200 radix-swipe-cancel:ease-[ease]',
								'focus-visible:ring-accent/75 border-2 border-white/10 shadow-2xl focus:outline-none focus-visible:ring'
							)}
						>
							<div className="flex">
								<div className="flex w-0 flex-1 items-center py-4 pl-5">
									<div className="radix w-full">
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
									<div className="flex flex-col space-y-1 px-3 py-2">
										<div className="flex h-0 flex-1">
											{toast.actionButton && (
												<ToastPrimitive.Action
													altText="view now"
													className="text-accent focus-visible:ring-accent/75 flex w-full items-center justify-center rounded-lg border border-transparent px-3 py-2 text-sm font-medium hover:bg-white/10 focus:z-10 focus:outline-none focus-visible:ring"
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
										<div className="flex h-0 flex-1">
											<ToastPrimitive.Close className="text-ink-faint focus-visible:ring-accent/75 flex w-full items-center justify-center rounded-lg border border-transparent px-3 py-2 text-sm font-medium hover:bg-white/10 focus:z-10 focus:outline-none focus-visible:ring">
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
