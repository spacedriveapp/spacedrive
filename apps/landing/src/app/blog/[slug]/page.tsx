import { allPosts } from '@contentlayer/generated';
import dayjs from 'dayjs';
import { Metadata } from 'next';
import { useMDXComponent } from 'next-contentlayer2/hooks';
import Image from 'next/image';
import { notFound } from 'next/navigation';
import { BlogTag } from '~/components/blog-tag';
import { BlogMDXComponents } from '~/components/mdx';

export function generateStaticParams(): Array<Props['params']> {
	return allPosts.map((post) => ({ slug: post.slug }));
}

interface Props {
	params: { slug: string };
}

export function generateMetadata({ params }: Props): Metadata {
	const post = allPosts.find((post) => post.slug === params.slug)!;
	const description =
		post.excerpt?.length || 0 > 160 ? post.excerpt?.substring(0, 160) + '...' : post.excerpt;

	return {
		title: `${post.title} - Spacedrive Blog`,
		description,
		authors: { name: post.author },
		openGraph: {
			title: post.title,
			description,
			images: post.image
		},
		twitter: {
			card: 'summary_large_image'
		}
	};
}

export default function Page({ params }: Props) {
	const post = allPosts.find((post) => post.slug === params.slug);

	if (!post) notFound();

	const MDXContent = useMDXComponent(post.body.code);

	return (
		<div className="lg:prose-xs container prose prose-invert m-auto mb-20 max-w-4xl p-5 pt-14">
			<>
				<figure>
					<Image
						src={post.image}
						alt={post.imageAlt ?? ''}
						className="mt-8 rounded-xl will-change-transform fade-in"
						height={400}
						width={900}
					/>
				</figure>
				<section className="flex flex-wrap gap-4 rounded-xl px-4">
					<div className="w-full grow">
						<h1 className="animation-delay-1 m-0 text-2xl leading-snug will-change-transform fade-in sm:text-4xl sm:leading-normal">
							{post.title}
						</h1>
						<p className="animation-delay-2 m-0 mt-2 will-change-transform fade-in">
							by <b>{post.author}</b> &middot; {dayjs(post.date).format('MM/DD/YYYY')}
						</p>
					</div>
					<div className="animation-delay-3 flex flex-wrap gap-2 will-change-transform fade-in">
						{post.tags.map((tag) => (
							<BlogTag key={tag} name={tag} />
						))}
					</div>
				</section>
				<article
					id="content"
					className="animation-delay-4 px-4 text-lg will-change-transform fade-in"
				>
					<MDXContent components={BlogMDXComponents} />
				</article>
			</>
		</div>
	);
}
