import clsx from 'clsx';
import Link from 'next/link';
import { Fragment } from 'react';

import { items } from './items';

export const metadata = {
	title: 'Roadmap - Spacedrive',
	description: 'What can Spacedrive do?'
};

export default function Page() {
	return (
		<div className="lg:prose-xs container prose prose-invert m-auto mb-20 mt-40 flex max-w-4xl flex-col gap-20 overflow-x-hidden p-4 md:overflow-visible">
			<section className="relative flex flex-col items-center">
				<div
					className="animation-delay-1 absolute top-[-1025px] size-[1800px] opacity-50 blur-sm duration-150 fade-in sm:right-[-400px] md:right-[-400px] lg:right-[-500px]"
					style={{
						backgroundImage: 'url(/images/roadmapbg.webp)',
						backgroundRepeat: 'no-repeat',
						backgroundSize: 'contain'
					}}
				/>
				<h1 className="fade-in-heading mb-0 text-center text-5xl leading-snug">
					What's next for Spacedrive?
				</h1>
				<p className="animation-delay-2 fade-in-heading text-center text-gray-400">
					Here is a list of the features we are working on, and the progress we have made
					so far.
				</p>
				<p>
					Last updated: <span className="font-bold text-white">January 21st, 2025</span>
				</p>
			</section>
			<section className="mt-32 grid auto-cols-auto grid-flow-row grid-cols-[auto_1fr] gap-x-4">
				{items.map((item, i) => (
					<Fragment key={i}>
						{/* Using span so i can use the group-last-of-type selector */}
						<span className="group flex max-w-40 items-start justify-end gap-4 first:items-start">
							<div
								className={clsx(
									'flex flex-col items-end',
									i === 0 ? '-translate-y-1/4' : '-translate-y-1/2'
								)}
							>
								{item?.subtext && (
									<span className="text-sm text-gray-300">{item?.subtext}</span>
								)}
								<h3
									className={
										`m-0 hidden text-right lg:block`
										// (i === 0 ? '-translate-y-1/4' : '-translate-y-1/2')
									}
								>
									{item.when}
								</h3>
							</div>
							<div className="flex h-full w-2 group-first:mt-2 group-first:rounded-t-full group-last-of-type:rounded-b-full lg:items-center">
								<div
									className={
										'flex h-full w-full ' +
										(item.completed ? 'z-10 bg-primary-500' : 'bg-gray-550')
									}
								>
									{item?.when !== undefined ? (
										<div
											className={clsx(
												'absolute z-20 mt-5 size-4 -translate-x-1/4 -translate-y-1/2 rounded-full border-2 border-gray-200 group-first:mt-0 group-first:self-start lg:mt-0',
												items[i - 1]?.completed || i === 0
													? 'z-10 bg-primary-500'
													: 'bg-gray-550'
											)}
										>
											&zwj;
										</div>
									) : (
										<div className="z-20">&zwj;</div>
									)}
								</div>
							</div>
						</span>
						<div className="group flex flex-col items-start justify-center gap-4">
							{item?.when && (
								<h3 className="mb-0 group-first-of-type:m-0 lg:hidden">
									{item.when}
								</h3>
							)}
							<div className="my-2 flex w-full flex-col space-y-2 rounded-xl border border-gray-500 p-4 group-last:mb-0 group-first-of-type:mt-0">
								<h3 className="m-0">{item.title}</h3>
								<p>{item.description}</p>
							</div>
						</div>
					</Fragment>
				))}
			</section>
			<section className="space-y-2 p-8 backdrop-blur-md">
				<h2 className="my-1">That's not all.</h2>
				<p>
					We're always open to ideas and feedback over{' '}
					<Link href="https://github.com/spacedriveapp/spacedrive/discussions">here</Link>{' '}
					and we have a <Link href="/blog">blog</Link> where you can find the latest news
					and updates.
				</p>
			</section>
		</div>
	);
}
