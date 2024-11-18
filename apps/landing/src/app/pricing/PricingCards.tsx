'use client';

import { Check } from '@phosphor-icons/react';
import { useState } from 'react';
import { Button, Switch } from '@sd/ui';

import { PlanAddons } from './PlanAddons';
import { plans, type Plan } from './plans';

export function PricingCards() {
	const [isAnnual, setIsAnnual] = useState(false);
	const [selectedPlan, setSelectedPlan] = useState<Plan | null>(null);

	return (
		<div className="mx-auto max-w-7xl px-4">
			<div className="fade-in-heading animation-delay-2 mx-auto mb-8 flex w-full items-center justify-center gap-3">
				<p className="text-sm font-medium text-white">Monthly</p>
				<Switch checked={isAnnual} onCheckedChange={setIsAnnual} size="lg" />
				<p className="text-sm font-medium text-white">
					Yearly <span className="text-primary-500">(20% off)</span>
				</p>
			</div>

			<div className="grid grid-cols-1 gap-6 lg:grid-cols-5">
				{plans.map((plan) => (
					<div
						key={plan.name}
						className={`rounded-xl border p-6 ${
							plan.name === 'Pro'
								? 'pro-card-border-gradient pro-card-shadow lg:col-span-2'
								: 'border-gray-500/50'
						}`}
					>
						<div className="flex flex-col">
							<h3 className="text-xl font-semibold text-white">{plan.name}</h3>
							{plan.price ? (
								<div className="mt-4">
									<span className="text-3xl font-bold text-white">
										${isAnnual ? plan.price.yearly : plan.price.monthly}
									</span>
									<span className="text-gray-400">/mo</span>
								</div>
							) : (
								<div className="mt-4">
									<span className="text-xl text-white">{plan.subTitle}</span>
								</div>
							)}

							<ul className="mt-6 space-y-4">
								{plan.features.map((feature) => (
									<li key={feature} className="flex items-start">
										<Check className="mr-2 h-5 w-5 flex-shrink-0 text-primary-500" />
										<span className="text-sm text-gray-300">{feature}</span>
									</li>
								))}
							</ul>

							<Button
								variant={plan.name === 'Pro' ? 'accent' : 'gray'}
								className="mt-8"
								onClick={() => setSelectedPlan(plan)}
							>
								{plan.name === 'Free' ? 'Get Started' : 'Select Plan'}
							</Button>
						</div>
					</div>
				))}
			</div>

			{selectedPlan && (
				<PlanAddons
					plan={selectedPlan}
					isAnnual={isAnnual}
					onClose={() => setSelectedPlan(null)}
				/>
			)}
		</div>
	);
}
