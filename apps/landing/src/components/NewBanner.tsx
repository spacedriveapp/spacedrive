export interface NewBannerProps {
	headline: string;
	href: string;
	link: string;
}

const NewBanner: React.FC<NewBannerProps> = (props) => {
	const { headline, href, link } = props;

	return (
		<aside
			onClick={() => (window.location.href = href)}
			className="fade-in-whats-new bg-opacity/50 hover:border-gray-550 hover:bg-gray-750 z-10 mb-5 flex w-10/12 cursor-pointer flex-row rounded-full border border-gray-600 bg-gray-800 px-5 py-1.5 text-xs transition sm:w-auto sm:text-base"
		>
			<strong className="text-gray-350 truncate font-semibold">{headline}</strong>
			<div role="separator" className="h-22 mx-4 w-[1px] bg-gray-500" />
			<a
				href={href}
				className="font-regular from-primary-400 decoration-primary-600 shrink-0 bg-gradient-to-r to-blue-600 bg-clip-text text-transparent"
			>
				{link} <span aria-hidden="true">&rarr;</span>
			</a>
		</aside>
	);
};

export default NewBanner;
