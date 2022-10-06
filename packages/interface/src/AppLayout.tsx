import * as ToastPrimitive from '@radix-ui/react-toast';
import { useCurrentLibrary } from '@sd/client';
import clsx from 'clsx';
import { Suspense, useEffect, useState } from 'react';
import { Outlet } from 'react-router-dom';

import { Sidebar } from './components/layout/Sidebar';
import { useOperatingSystem } from './hooks/useOperatingSystem';
import { useToasts } from './hooks/useToasts';

export function AppLayout() {
	const { libraries } = useCurrentLibrary();
	const os = useOperatingSystem();

	// This will ensure nothing is rendered while the `useCurrentLibrary` hook navigates to the onboarding page. This prevents requests with an invalid library id being sent to the backend
	if (libraries?.length === 0) {
		return null;
	}

	return (
		<div
			onContextMenu={(e) => {
				// TODO: allow this on some UI text at least / disable default browser context menu
				e.preventDefault();
				return false;
			}}
			className={clsx(
				'flex flex-row h-screen overflow-hidden text-gray-900 select-none dark:text-white cursor-default',
				os === 'macOS' && 'rounded-xl',
				os !== 'browser' && os !== 'windows' && 'border border-gray-200 dark:border-gray-500'
			)}
		>
			<Sidebar />
			<div className="relative flex w-full h-screen max-h-screen bg-white dark:bg-gray-650">
				<Suspense fallback={<p>Loading...</p>}>
					<Outlet />
				</Suspense>
			</div>
			<Toasts />
		</div>
	);
}

function Toasts() {
	const { toasts, addToast, removeToast } = useToasts();

	// useEffect(() => {
	// 	setTimeout(() => {
	// 		addToast({
	// 			title: 'Spacedrop',
	// 			subtitle: 'Someone tried to send you a file. Accept it?',
	// 			actionButton: {
	// 				text: 'Accept',
	// 				onClick: () => {
	// 					console.log('Bruh');
	// 				}
	// 			}
	// 		});
	// 	}, 2000);
	// }, []);

	return (
		<div className="fixed flex right-0">
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
								'bg-gray-800/20 backdrop-blur',
								'radix-state-open:animate-toast-slide-in-bottom md:radix-state-open:animate-toast-slide-in-right',
								'radix-state-closed:animate-toast-hide',
								'radix-swipe-end:animate-toast-swipe-out',
								'translate-x-radix-toast-swipe-move-x',
								'radix-swipe-cancel:translate-x-0 radix-swipe-cancel:duration-200 radix-swipe-cancel:ease-[ease]',
								'focus:outline-none focus-visible:ring focus-visible:ring-primary focus-visible:ring-opacity-75 border-white/10 border-2 shadow-2xl'
							)}
						>
							<div className="flex">
								<div className="w-0 flex-1 flex items-center pl-5 py-4">
									<div className="w-full radix">
										<ToastPrimitive.Title className="text-sm font-medium text-gray-900 dark:text-gray-100">
											{toast.title}
										</ToastPrimitive.Title>
										{toast.subtitle && (
											<ToastPrimitive.Description className="mt-1 text-sm text-gray-700 dark:text-gray-400">
												{toast.subtitle}
											</ToastPrimitive.Description>
										)}
									</div>
								</div>
								<div className="flex">
									<div className="flex flex-col px-3 py-2 space-y-1">
										<div className="h-0 flex-1 flex">
											{toast.actionButton && (
												<ToastPrimitive.Action
													altText="view now"
													className="w-full border border-transparent rounded-lg px-3 py-2 flex items-center justify-center text-sm font-medium text-primary dark:text-primary hover:bg-white/10 focus:z-10 focus:outline-none focus-visible:ring focus-visible:ring-primary focus-visible:ring-opacity-75"
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
										<div className="h-0 flex-1 flex">
											<ToastPrimitive.Close className="w-full border border-transparent rounded-lg px-3 py-2 flex items-center justify-center text-sm font-medium text-gray-700 dark:text-gray-100 hover:bg-white/10 focus:z-10 focus:outline-none focus-visible:ring focus-visible:ring-primary focus-visible:ring-opacity-75">
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
