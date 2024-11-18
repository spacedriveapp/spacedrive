export interface Plan {
	name: string;
	subTitle?: string;
	price?: {
		monthly: number;
		yearly: number;
	};
	features: string[];
}

export const plans: Plan[] = [
	{
		name: 'Free',
		subTitle: 'Free forever',
		features: [
			'Local storage only',
			'Cross-platform support',
			'End-to-end encryption',
			'2 cloud devices',
			'100 MB cloud database sync',
			'7 days version history'
		]
	},
	{
		name: 'Personal',
		price: {
			monthly: 5.99,
			yearly: 4.79
		},
		features: [
			'1 TB storage',
			'Unlimited cloud database sync',
			'5 cloud devices',
			'3 shares',
			'Cold storage redundancy',
			'30 days version history'
		]
	},
	{
		name: 'Pro',
		price: {
			monthly: 19.99,
			yearly: 15.99
		},
		features: [
			'5 TB storage',
			'Unlimited cloud devices',
			'Unlimited shares',
			'Custom branding',
			'Priority support',
			'90 days version history'
		]
	},
	{
		name: 'Business',
		price: {
			monthly: 39.99,
			yearly: 31.99
		},
		features: [
			'10 TB storage',
			'Up to 10 team members',
			'SSO integration',
			'Advanced security',
			'Priority phone support',
			'180 days version history'
		]
	}
];
