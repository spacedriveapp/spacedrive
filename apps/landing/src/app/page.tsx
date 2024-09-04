import { Assistant, Explorer, Features, Github, Header, Search } from '~/app/page-sections';

export default function Page() {
	return (
		<div className="flex flex-col gap-12 md:gap-[200px]">
			<Header />
			<Explorer />
			<Features />
			<Search />
			<Assistant />
			<Github />
		</div>
	);
}
