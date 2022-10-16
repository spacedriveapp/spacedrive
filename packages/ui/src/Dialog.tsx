import * as DialogPrimitive from '@radix-ui/react-dialog';
import clsx from 'clsx';
import { ReactNode, useState } from 'react';
import { animated, config, useTransition } from 'react-spring';

import { Button, Loader } from '../';

export interface DialogProps extends DialogPrimitive.DialogProps {
	trigger: ReactNode;
	ctaLabel?: string;
	ctaDanger?: boolean;
	ctaAction?: () => void;
	title?: string;
	description?: string;
	children?: ReactNode;
	transformOrigin?: string;
	loading?: boolean;
	submitDisabled?: boolean;
}

export function Dialog(props: DialogProps) {
	const [open, setOpen] = useState(false);

	const transitions = useTransition(open, {
		from: {
			opacity: 0,
			transform: `translateY(20px)`,
			transformOrigin: props.transformOrigin || 'bottom'
		},
		enter: { opacity: 1, transform: `translateY(0px)` },
		leave: { opacity: 0, transform: `translateY(20px)` },
		config: { mass: 0.4, tension: 200, friction: 10 }
	});

	return (
		<DialogPrimitive.Root open={open} onOpenChange={setOpen}>
			<DialogPrimitive.Trigger asChild>{props.trigger}</DialogPrimitive.Trigger>
			{transitions(
				(styles, show) =>
					show && (
						<DialogPrimitive.Portal forceMount>
							<DialogPrimitive.Overlay asChild>
								<animated.div
									className="fixed top-0 bottom-0 left-0 right-0 z-50 grid overflow-y-auto bg-black bg-opacity-50 rounded-xl place-items-center m-[1px]"
									style={{
										opacity: styles.opacity
									}}
								>
									<DialogPrimitive.Content forceMount asChild>
										<animated.div
											style={styles}
											className="min-w-[300px] max-w-[400px] rounded-md bg-gray-650 text-white border border-gray-550 shadow-deep"
										>
											<form onSubmit={(e) => e.preventDefault()}>
												<div className="p-5">
													<DialogPrimitive.Title className="mb-2 font-bold">
														{props.title}
													</DialogPrimitive.Title>
													<DialogPrimitive.Description className="text-sm text-gray-300">
														{props.description}
													</DialogPrimitive.Description>
													{props.children}
												</div>
												<div className="flex flex-row justify-end px-3 py-3 space-x-2 bg-gray-600 border-t border-gray-550">
													{props.loading && <Loader />}
													<div className="flex-grow" />
													<DialogPrimitive.Close asChild>
														<Button
															loading={props.loading}
															disabled={props.loading}
															size="sm"
															variant="gray"
														>
															Close
														</Button>
													</DialogPrimitive.Close>
													<Button
														onClick={props.ctaAction}
														type="submit"
														size="sm"
														loading={props.loading}
														disabled={props.loading || props.submitDisabled}
														variant={props.ctaDanger ? 'colored' : 'primary'}
														className={clsx(props.ctaDanger && 'bg-red-500 border-red-500')}
													>
														{props.ctaLabel}
													</Button>
												</div>
											</form>
										</animated.div>
									</DialogPrimitive.Content>
								</animated.div>
							</DialogPrimitive.Overlay>
						</DialogPrimitive.Portal>
					)
			)}
		</DialogPrimitive.Root>
	);
}
