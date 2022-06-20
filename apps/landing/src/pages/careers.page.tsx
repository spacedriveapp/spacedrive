import {
	ClockIcon,
	CurrencyDollarIcon,
	DesktopComputerIcon,
	EmojiHappyIcon,
	HeartIcon,
	HomeIcon,
	LightningBoltIcon,
	StarIcon,
	TrendingUpIcon
} from '@heroicons/react/outline';
import { Button } from '@sd/ui';
import { Heartbeat } from 'phosphor-react';
import React from 'react';
import { Helmet } from 'react-helmet';
import { ReactComponent as Content } from '~/docs/changelog/index.md';

const positions = [
	{
		name: 'TypeScript React UI/UX Engineer',
		type: 'Full-time',
		salary: '$80k - $120k',
		description: `You'll build the primary desktop interface for Spacedrive in React, with TypeScript and Tailwind. You'll need an eye for design as well as a solid understanding of the React ecosystem.`
	},
	{
		name: 'Rust Backend Engineer',
		type: 'Full-time',
		salary: '$80k - $120k',
		description: `You'll build out our Rust core, the decentralized backend that powers our app. From the virtual filesystem to encryption and search. You'll need to live and breathe Rust, not be afraid to get low-level.`
	},
	{
		name: 'TypeScript React Native Engineer',
		type: 'Full-time',
		salary: '$80k - $120k',
		description: `You'll build out the majority of our mobile app in TypeScript and React Native. Developing a mobile first component library based on the design of our desktop application. You'll need to be passionate for building React Native apps that look and feel native.`
	}
];
const values = [
	{
		title: 'Async',
		desc: 'To accommodate our international team and community, we work and communicate asynchronously.',
		icon: ClockIcon
	},
	{
		title: 'Quality',
		desc: 'From our interface design to our code, we strive to build software that will last.',
		icon: StarIcon
	},
	{
		title: 'Speed',
		desc: 'We get things done quickly, through small iteration cycles and frequent updates.',
		icon: LightningBoltIcon
	},
	{
		title: 'Transparency',
		desc: 'We are human beings that make mistakes, but through total transparency we can solve them faster.',
		icon: HeartIcon
	}
];

const perks = [
	{
		title: 'Competitive Salary',
		desc: `We want the best, and will pay for the best. If you shine through we'll make sure you're paid what you're worth.`,
		icon: CurrencyDollarIcon,
		color: '#0DD153'
	},
	{
		title: 'Stock Options',
		desc: `As an early employee, you deserve to own a piece of our company. Stock options will be offered as part of your onboarding process.`,
		icon: TrendingUpIcon,
		color: '#BD0DD1'
	},
	{
		title: 'Paid Time Off',
		desc: `Rest is important, you deliver your best work when you've had your downtime. We offer 2 weeks paid time off per year, and if you need more, we'll give you more.`,
		icon: EmojiHappyIcon,
		color: '#9210FF'
	},
	{
		title: 'Work From Home',
		desc: `As an open source project, we're remote first and intend to keep it that way. Sorry Elon.`,
		icon: HomeIcon,
		color: '#D1A20D'
	},
	{
		title: 'Desk Budget',
		desc: `Need an M1 MacBook Pro? We've got you covered. (You'll probably need one with Rust compile times)`,
		icon: DesktopComputerIcon,
		color: '#0DC5D1'
	},
	{
		title: 'Health Care',
		desc: `We use Deel for hiring and payroll, all your health care needs are covered.`,
		icon: HeartIcon,
		color: '#D10D7F'
	}
];

function Page() {
	const openPositionsRef = React.useRef<HTMLHRElement>(null);
	const scrollToPositions = () => openPositionsRef.current?.scrollIntoView({ behavior: 'smooth' });

	return (
		<>
			<Helmet>
				<title>Careers - Spacedrive</title>
				<meta name="description" content="Work with us to build the future of file management." />
			</Helmet>
			<div className="container relative max-w-4xl min-h-screen p-4 m-auto mt-32 mb-20 prose text-white prose-invert">
				<div
					className="bloom subtle egg-bloom-two -top-60 -right-[400px]"
					style={{ transform: 'scale(2)' }}
				/>
				<h1 className="px-2 mb-3 text-4xl font-black leading-tight text-center text-white fade-in-heading md:text-5xl">
					Build the future of files.
				</h1>
				<div className="z-30 flex flex-col items-center fade-in animation-delay-1">
					<p className="z-40 text-lg text-center text-gray-350">
						Spacedrive is redefining the way we think about our personal data, building a open
						ecosystem to help preserve your digital legacy and make cross-platform file management a
						breeze.
					</p>
					<Button
						onClick={scrollToPositions}
						className="z-30 border-0 cursor-pointer"
						variant="primary"
					>
						See Open Positions
					</Button>
					<hr className="w-full my-24 border-gray-200 opacity-10 border-1" />
					<h1 className="px-2 mb-0 text-4xl font-black leading-tight text-center">Our Values</h1>
					<p className="mt-2 mb-4">What drives us daily.</p>
					<div className="grid w-full grid-cols-1 gap-4 mt-5 sm:grid-cols-2">
						{values.map((value) => (
							<div className="flex flex-col p-10 bg-opacity-50 border border-gray-500 rounded-md bg-gray-550">
								<value.icon className="w-8 m-0" />
								<h2 className="mt-4 mb-1">{value.title}</h2>
								<p className="mt-1 mb-0 text-gray-350">{value.desc}</p>
							</div>
						))}
					</div>
					<hr className="w-full my-24 border-gray-200 opacity-10 border-1" />
					<h1 className="px-2 mb-0 text-4xl font-black leading-tight text-center text-white">
						Perks and Benefits
					</h1>
					<p className="mt-2 mb-4">We're behind you 100%.</p>
					<div className="grid w-full grid-cols-1 gap-4 mt-5 sm:grid-cols-3">
						{perks.map((value) => (
							<div
								style={{ backgroundColor: value.color + '10', borderColor: value.color + '30' }}
								className="flex flex-col p-8 border rounded-md bg-gray-550 bg-opacity-30"
							>
								<value.icon className="w-8 m-0" color={value.color} />
								<h3 className="mt-4 mb-1">{value.title}</h3>
								<p className="mt-1 mb-0 text-sm text-white opacity-60">{value.desc}</p>
							</div>
						))}
					</div>
					<hr className="w-full my-24 border-gray-200 opacity-10 border-1" ref={openPositionsRef} />
					<h1 className="px-2 mb-0 text-4xl font-black leading-tight text-center text-white">
						Open Positions
					</h1>
					<p className="mt-2 mb-4">Any of these suit you? Apply now!</p>
					<div className="grid w-full grid-cols-1 gap-4 mt-5">
						{positions.map((value) => (
							<div className="flex flex-col p-10 bg-opacity-50 border border-gray-500 rounded-md bg-gray-550">
								<div className="flex flex-col sm:flex-row">
									<h2 className="m-0">{value.name}</h2>
									<div className="mt-3 sm:mt-0.5">
										<span className="text-sm font-semibold text-gray-300 sm:ml-4">
											<CurrencyDollarIcon className="inline w-4 mr-1 -mt-1" />
											{value.salary}
										</span>
										<span className="ml-4 text-sm font-semibold text-gray-300">
											<ClockIcon className="inline w-4 mr-1 -mt-1" />
											{value.type}
										</span>
									</div>
								</div>
								<p className="mt-3 mb-0 text-gray-350">{value.description}</p>
							</div>
						))}
					</div>
					<hr className="w-full my-24 border-gray-200 opacity-10 border-1" />
					<h1 className="px-2 mb-0 text-3xl font-black leading-tight text-center text-white">
						How to apply?
					</h1>
					<p>
						Send your cover letter and resume to <b>careers at spacedrive dot com</b> and we'll get
						back to you shortly!
					</p>
				</div>
			</div>
		</>
	);
}

export default Page;
