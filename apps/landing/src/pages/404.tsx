import { SmileyXEyes } from '@phosphor-icons/react';
import Head from 'next/head';
import { useRouter } from 'next/router';
import { Button } from '@sd/ui';
import Markdown from '~/components/Markdown';
import PageWrapper from '~/components/PageWrapper';

export default function Custom404Page() {
	const router = useRouter();
	return (
		<PageWrapper>
			<Markdown classNames="flex w-full justify-center">
				<Head>
					<title>Not Found - Spacedrive</title>
				</Head>
				<div className="m-auto flex flex-col items-center ">
					<div className="h-32" />
					<SmileyXEyes className="mb-3 h-44 w-44" />
					<h1 className="mb-2 text-center">
						In the quantum realm this page potentially exists.
					</h1>
					<p>In other words, thats a 404.</p>
					<div className="flex flex-wrap justify-center">
						<Button
							onClick={() => router.back()}
							className="mr-3 mt-2 cursor-pointer "
							variant="gray"
						>
							← Back
						</Button>
						<Button
							href="/"
							className="mt-2 cursor-pointer !text-white"
							variant="accent"
						>
							Discover Spacedrive →
						</Button>
					</div>
				</div>
				<div className="h-80" />
			</Markdown>
		</PageWrapper>
	);
}
