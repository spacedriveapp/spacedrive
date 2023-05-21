import { Footer } from './Footer';
import NavBar from './NavBar';

export default function PageWrapper({ children }: { children: React.ReactNode }) {
	return (
		<>
			<NavBar />
			<main className="dark z-10 m-auto max-w-[100rem] overflow-hidden dark:bg-black dark:text-white">
				{children}
			</main>
			<Footer />
		</>
	);
}
