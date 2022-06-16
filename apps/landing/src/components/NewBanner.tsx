import React from 'react';

export interface NewBannerProps {
	headline: string;
	href: string;
	link: string;
}

const NewBanner: React.FC<NewBannerProps> = (props) => {
	const { headline, href, link } = props;

	return (
		<aside className="text-xs w-10/12 sm:w-auto sm:text-base cursor-default fade-in-whats-new px-5 py-1.5 bg-opacity-50 mb-5 flex flex-row bg-gray-800 hover:bg-gray-750 border border-gray-600 hover:border-gray-550 rounded-full transition">
			<strong className="font-semibold truncate text-gray-350">{headline}</strong>
			<div role="separator" className="w-[1px] mx-4 h-22 bg-gray-500" />
			<a
				href={href}
				className="flex-shrink-0 text-transparent font-regular bg-clip-text bg-gradient-to-r from-primary-400 to-blue-600 decoration-primary-600 hover:underline"
			>
				{link} <span aria-hidden="true">&rarr;</span>
			</a>
		</aside>
	);
};

export default NewBanner;
