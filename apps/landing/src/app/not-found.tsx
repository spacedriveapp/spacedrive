'use client';

import { SmileyXEyes } from '@phosphor-icons/react';
import { useRouter } from 'next/navigation';
import { Button } from '@sd/ui';
import { MarkdownPage } from '~/components/markdown-page';

export const metadata = {
	title: 'Not Found - Spacedrive'
};

export default function NotFound() {
	const router = useRouter();

	return (
		<MarkdownPage classNames="flex w-full justify-center">
			<div className="m-auto flex flex-col items-center">
				<div className="h-32" />
				<SmileyXEyes className="mb-3 size-44" />
				<h1 className="mb-2 text-center">
					In the quantum realm this page potentially exists.
				</h1>
				<p>In other words, thats a 404.</p>
				<div className="flex flex-wrap justify-center">
					<Button
						className="mr-3 mt-2 cursor-pointer"
						variant="gray"
						onClick={() => router.back()}
					>
						← Back
					</Button>
					<Button href="/" className="mt-2 cursor-pointer !text-white" variant="accent">
						Discover Spacedrive →
					</Button>
				</div>
			</div>
			<div className="h-80" />
		</MarkdownPage>
	);
}
