import { ArrowDown, Clock, CurrencyDollar } from '@phosphor-icons/react/dist/ssr';
import { CtaSecondaryButton } from '~/components/cta-secondary-button';

import { perks, positions, values } from './data';

export const metadata = {
	title: 'Careers - Spacedrive',
	description: 'Work with us to build the future of file management.'
};

export default function CareersPage() {
	return (
		<div className="container prose prose-invert relative m-auto mb-20 mt-40 min-h-screen max-w-4xl overflow-x-hidden p-4 text-white md:overflow-visible">
			<div className="relative mb-[200px] flex flex-col items-center justify-center overflow-visible xs:mb-32 sm:mb-40 lg:mb-40">
				<div
					className="animation-delay-1 absolute right-[-750px] top-[-1000px] mx-auto size-[1700px] blur-sm duration-150 fade-in xs:top-[-1150px] sm:top-[-1200px] md:top-[-1150px]"
					style={{
						backgroundImage: 'url(/images/careersbg.webp',
						backgroundRepeat: 'no-repeat',
						backgroundSize: 'cover',
						backgroundPosition: '-300px 40px'
					}}
				/>
				<h1 className="fade-in-heading mb-3 px-2 text-center text-4xl font-black leading-tight text-white md:text-5xl">
					Build the future of files.
				</h1>
				<p className="animation-delay-1 fade-in-heading z-40 text-center text-lg text-gray-350">
					Spacedrive is redefining the way we think about our personal data, building a
					open ecosystem to help preserve your digital legacy and make cross-platform file
					management a breeze.
				</p>
				<CtaSecondaryButton
					icon={<ArrowDown weight="bold" />}
					href="#open-positions"
					className="fade-in-heading animation-delay-2 z-30 mt-8 min-w-fit cursor-pointer !gap-x-1.5 border-0"
				>
					Open Positions
				</CtaSecondaryButton>
			</div>
			<div className="animation-delay-1 z-30 flex flex-col items-center fade-in">
				<h2 className="mb-0 px-2 text-center text-4xl font-black leading-tight">
					Our Values
				</h2>
				<p className="mb-4 mt-2">What drives us daily.</p>
				<div className="mt-5 grid w-full grid-cols-1 gap-4 sm:grid-cols-2">
					{values.map((value, index) => (
						<div
							key={value.title + index}
							className="bento-border-left relative flex flex-col rounded-[10px] bg-[radial-gradient(66.79%_83.82%_at_0%_3.69%,#1B1D25_0%,#15161C_100%)] p-10"
						>
							<value.icon
								width="1em"
								height="1em"
								className="text-[32px]"
								weight="bold"
							/>
							<h3 className="mb-1 mt-4 text-2xl font-bold leading-snug">
								{value.title}
							</h3>
							<p className="mb-0 mt-1 text-gray-350">{value.desc}</p>
						</div>
					))}
				</div>
				<hr className="border-1 my-24 w-full border-gray-200 opacity-10" />
				<h2 className="mb-0 px-2 text-center text-4xl font-black leading-tight text-white">
					Perks and Benefits
				</h2>
				<p className="mb-4 mt-2">We're behind you 100%.</p>
				<div className="mt-5 grid w-full grid-cols-1 gap-4 sm:grid-cols-3">
					{perks.map((value, index) => (
						<div
							key={value.title + index}
							style={{
								backgroundColor: value.color + '10',
								borderColor: value.color + '30'
							}}
							className="flex flex-col rounded-md border bg-gray-550/30 p-8"
						>
							<value.icon
								width="1em"
								height="1em"
								className="text-[32px]"
								weight="bold"
								color={value.color}
							/>
							<h3 className="mb-1 mt-4">{value.title}</h3>
							<p className="mb-0 mt-1 text-sm text-white opacity-60">{value.desc}</p>
						</div>
					))}
				</div>
				<hr className="border-1 my-24 w-full border-gray-200 opacity-10" />
				<h2
					id="open-positions"
					className="mb-0 px-2 text-center text-4xl font-black leading-tight text-white"
				>
					Open Positions
				</h2>
				{positions.length === 0 ? (
					<p className="mt-2 text-center text-gray-350">
						There are no positions open at this time. Please check back later!
					</p>
				) : (
					<>
						<p className="mb-4 mt-2">If any open positions suit you, apply now!</p>
						<div className="mt-5 grid w-full grid-cols-1 gap-4">
							{positions.map((value, index) => (
								<div
									key={value.name + index}
									className="flex flex-col rounded-md border border-gray-500 bg-gray-550/50 p-10"
								>
									<div className="flex flex-col sm:flex-row">
										<h3 className="m-0 text-2xl leading-tight">{value.name}</h3>
										<div className="mt-3 sm:mt-0.5">
											<span className="text-sm font-semibold text-gray-300 sm:ml-4">
												<CurrencyDollar className="-mt-1 mr-1 inline w-4" />
												{value.salary}
											</span>
											<span className="ml-4 text-sm font-semibold text-gray-300">
												<Clock className="-mt-1 mr-1 inline w-4" />
												{value.type}
											</span>
										</div>
									</div>
									<p className="mb-0 mt-3 text-gray-350">{value.description}</p>
								</div>
							))}
						</div>
					</>
				)}

				<hr className="border-1 my-24 w-full border-gray-200 opacity-10" />
				<h2 className="mb-0 px-2 text-center text-3xl font-black text-white">
					How to apply?
				</h2>
				<p className="mt-2">
					Send your cover letter and resume to{' '}
					<strong>careers at spacedrive dot com</strong> and we'll get back to you
					shortly!
				</p>
			</div>
		</div>
	);
}
