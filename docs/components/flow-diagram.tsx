interface FlowStep {
	title: string;
	description?: string;
	items?: string[];
	metrics?: Record<string, string>;
}

export function FlowDiagram({steps = []}: {steps?: FlowStep[]}) {
	return (
		<div className="my-8 space-y-0">
			{steps.map((step, index) => (
				<div key={index} className="flex flex-col items-center">
					<div className="border-fd-primary/20 from-fd-primary/5 hover:border-fd-primary/40 w-full max-w-2xl rounded-lg border bg-gradient-to-br to-transparent p-6 shadow-sm backdrop-blur-sm transition-all hover:shadow-md">
						<div className="flex items-start gap-4">
							<div className="bg-fd-primary/20 text-fd-primary flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-sm font-semibold">
								{index + 1}
							</div>
							<div className="flex-1">
								<h3 className="text-fd-foreground mt-0 text-base font-semibold">
									{step.title}
								</h3>
								{step.description && (
									<p className="text-fd-muted-foreground mt-1 text-sm leading-relaxed">
										{step.description}
									</p>
								)}
								{step.items && (
									<div className="mt-3 flex flex-wrap gap-2">
										{step.items.map((item, i) => (
											<span
												key={i}
												className="border-fd-primary/30 bg-fd-primary/10 text-fd-muted-foreground rounded-md border px-2.5 py-1 text-xs"
											>
												{item}
											</span>
										))}
									</div>
								)}
								{step.metrics && (
									<div className="mt-3 flex flex-wrap gap-3 text-xs">
										{Object.entries(step.metrics).map(
											([key, value]) => (
												<div
													key={key}
													className="bg-fd-muted rounded-md px-2.5 py-1.5 font-mono"
												>
													<span className="text-fd-muted-foreground">
														{key}:
													</span>{' '}
													<span className="text-fd-primary font-semibold">
														{value}
													</span>
												</div>
											)
										)}
									</div>
								)}
							</div>
						</div>
					</div>
					{index < steps.length - 1 && (
						<div className="relative flex h-12 w-0.5 items-center justify-center">
							<div className="from-fd-primary/40 via-fd-primary/20 to-fd-primary/40 h-full w-full bg-gradient-to-b" />
							<div className="bg-fd-primary/60 absolute h-2 w-2 rounded-full" />
						</div>
					)}
				</div>
			))}
		</div>
	);
}
