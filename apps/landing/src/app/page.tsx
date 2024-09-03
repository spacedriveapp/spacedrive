import {
	Assistant,
	Explorer,
	Features,
	Github,
	Header,
	Search
} from '~/components/landing-sections';

export default function Page() {
	return (
		<div className="flex flex-col gap-[200px]">
			<Header />
			<Explorer />
			<Features />
			<Search />
			<Assistant />
			<Github />
		</div>
	);
}
