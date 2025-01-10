export function Companies() {
	const companies = [
		'Microsoft',
		'Apache',
		'Cloudflare',
		'Google',
		'CrabNebula',
		'Nvidia',
		'Gitpod',
		'PostHog',
		'Github',
		'1Password',
		'Netflix',
		'Datadog',
		'RunPod',
		'ClickHouse'
	];

	// Duplicate the array to create a seamless loop
	const duplicatedCompanies = [...companies, ...companies];

	return (
		<section className="container mx-auto px-4 pt-24">
			<h2 className="mb-12 text-center text-3xl font-bold text-white">
				Loved by employees at leading tech companies
			</h2>
			<div className="relative">
				{/* Gradient masks for fade effect */}
				<div className="pointer-events-none absolute inset-y-0 left-0 z-10 w-32 bg-gradient-to-r from-[#09090b] to-transparent" />
				<div className="pointer-events-none absolute inset-y-0 right-0 z-10 w-32 bg-gradient-to-l from-[#09090b] to-transparent" />

				{/* Scrolling container */}
				<div className="flex overflow-hidden">
					<div className="animate-scroll flex gap-12 py-4">
						{duplicatedCompanies.map((company, index) => (
							<div
								key={`${company}-${index}`}
								className="flex h-12 w-24 shrink-0 items-center justify-center opacity-70 transition-opacity hover:opacity-100 md:h-16 md:w-32"
							>
								{/* eslint-disable-next-line @next/next/no-img-element */}
								<img
									src={`/images/companies/${company}.svg`}
									alt={`${company} logo`}
									className="max-h-full max-w-full"
								/>
							</div>
						))}
					</div>
				</div>
			</div>
		</section>
	);
}
