import {
	Clock,
	CurrencyDollar,
	Desktop,
	Heart,
	House,
	LightningSlash,
	Smiley,
	Star,
	TrendUp
} from '@phosphor-icons/react/dist/ssr';

export interface PositionPosting {
	name: string;
	type: string;
	salary: string;
	description: string;
}

export const positions: PositionPosting[] = [
	{
		name: 'Core Engineer',
		type: 'Full-time',
		salary: '$140,000 - $180,000',
		description:
			"Lead development of Spacedrive's core storage engine and distributed filesystem in Rust. You'll architect and implement critical systems including storage classification, encrypted data sync, and filesystem operations. Deep knowledge of systems programming, distributed systems, and Rust required. Experience with FUSE filesystems and cloud storage systems highly valued."
	},
	{
		name: 'Platform Engineer',
		type: 'Full-time',
		salary: '$130,000 - $170,000',
		description:
			"Own Spacedrive's cross-platform client applications built with Tauri and React. Focus on performance optimization, native platform integration, and smooth user experience across desktop and mobile. Strong TypeScript/React experience required, with knowledge of Rust and native platform APIs (Windows, macOS, Linux, iOS, Android) highly valued."
	},
	{
		name: 'Cloud Infrastructure Engineer',
		type: 'Full-time',
		salary: '$150,000 - $190,000',
		description:
			"Design and implement Spacedrive's cloud storage infrastructure, focusing on our innovative storage architecture. Build highly scalable systems for data ingestion, storage optimization, and content delivery. Deep experience with cloud infrastructure (AWS/GCP), object storage, and distributed systems required. Knowledge of Rust and TypeScript preferred."
	},
	{
		name: 'Security Engineer',
		type: 'Full-time',
		salary: '$160,000 - $200,000',
		description:
			"Lead security architecture for Spacedrive's zero-knowledge encrypted storage platform. Design and implement end-to-end encryption, secure key management, and audit systems. Expertise in cryptography, secure systems design, and penetration testing required. Experience with Rust and secure storage systems highly valued."
	}
];

export const values = [
	{
		title: 'Async',
		desc: 'To accommodate our international team and community, we work and communicate asynchronously.',
		icon: Clock
	},
	{
		title: 'Quality',
		desc: 'From our interface design to our code, we strive to build software that will last.',
		icon: Star
	},
	{
		title: 'Speed',
		desc: 'We get things done quickly, through small iteration cycles and frequent updates.',
		icon: LightningSlash
	},
	{
		title: 'Transparency',
		desc: 'We are human beings that make mistakes, but through total transparency we can solve them faster.',
		icon: Heart
	}
];

export const perks = [
	{
		title: 'Competitive Salary',
		desc: `We want the best, and will pay for the best. If you shine through we'll make sure you're paid what you're worth.`,
		icon: CurrencyDollar,
		color: '#0DD153'
	},
	{
		title: 'Stock Options',
		desc: `As an early employee, you deserve to own a piece of our company. Stock options will be offered as part of your onboarding process.`,
		icon: TrendUp,
		color: '#BD0DD1'
	},
	{
		title: 'Paid Time Off',
		desc: `Rest is important, you deliver your best work when you've had your downtime. We offer 4 weeks paid time off per year, and if you need more, we'll give you more.`,
		icon: Smiley,
		color: '#9210FF'
	},
	{
		title: 'Work From Home',
		desc: `As an open source project, we're remote first and intend to keep it that way. Sorry Elon.`,
		icon: House,
		color: '#D1A20D'
	},
	{
		title: 'Desk Budget',
		desc: `Need an M1 MacBook Pro? We've got you covered. (You'll probably need one with Rust compile times)`,
		icon: Desktop,
		color: '#0DC5D1'
	},
	{
		title: 'Health Care',
		desc: `We use Deel for hiring and payroll, all your health care needs are covered.`,
		icon: Heart,
		color: '#D10D7F'
	}
];
