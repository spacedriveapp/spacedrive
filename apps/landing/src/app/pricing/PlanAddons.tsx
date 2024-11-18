'use client';

import { X } from '@phosphor-icons/react';
import { Button, Dialog } from '@sd/ui';

import type { Plan } from './plans';

interface PlanAddonsProps {
	plan: Plan;
	isAnnual: boolean;
	onClose: () => void;
}

export function PlanAddons({ plan, isAnnual, onClose }: PlanAddonsProps) {
	return (
		<Dialog open onOpenChange={() => onClose()}>
			<div className="fixed inset-0 z-50 bg-black/50 backdrop-blur-sm">
				<div className="fixed left-1/2 top-1/2 w-full max-w-2xl -translate-x-1/2 -translate-y-1/2 rounded-xl bg-gray-850 p-6">
					<div className="mb-6 flex items-center justify-between">
						<h2 className="text-2xl font-bold text-white">
							Customize your {plan.name} plan
						</h2>
						<button onClick={onClose}>
							<X className="h-6 w-6 text-gray-400" />
						</button>
					</div>

					<div className="space-y-6">
						<div className="rounded-lg border border-gray-500/50 p-4">
							<h3 className="mb-4 text-lg font-semibold text-white">
								Data Retention Add-on
							</h3>
							<div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
								{[1, 2, 3, 4, 5].map((years) => (
									<label
										key={years}
										className="relative flex cursor-pointer rounded-lg border border-gray-500/50 p-4 hover:bg-gray-800/50"
									>
										<input
											type="radio"
											name="retention"
											className="peer sr-only"
										/>
										<div className="flex flex-col">
											<span className="text-sm font-medium text-white">
												{years} Year{years > 1 ? 's' : ''}
											</span>
											<span className="text-sm text-gray-400">
												${(years * 36).toFixed(2)}
											</span>
										</div>
									</label>
								))}
							</div>
						</div>

						<div className="flex justify-end gap-4">
							<Button variant="gray" onClick={onClose}>
								Cancel
							</Button>
							<Button variant="accent">Continue to checkout</Button>
						</div>
					</div>
				</div>
			</div>
		</Dialog>
	);
}
