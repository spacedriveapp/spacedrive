import { allPosts } from '@contentlayer/generated';
import { InferGetStaticPropsType } from 'next';
import { useMDXComponent } from 'next-contentlayer/hooks';
import Head from 'next/head';
import Image from 'next/image';

import { BlogTag } from '~/components/BlogTag';
import { BlogMDXComponents } from '~/components/mdx';
import PageWrapper from '~/components/PageWrapper';

export async function getStaticPaths() {
	const paths = allPosts.map((post) => post.url);
	return {
		paths,
		fallback: false
	};
}

export async function getStaticProps({ params }: { params: { slug: string } }) {
	const post = allPosts.find((post) => post.slug === params.slug);

	if (!post) {
		return {
			notFound: true
		};
	}

	return {
		props: {
			post
		}
	};
}

export default function PostPage({ post }: InferGetStaticPropsType<typeof getStaticProps>) {
	const MDXContent = useMDXComponent(post.body.code);

	const description =
		post.excerpt?.length || 0 > 160 ? post.excerpt?.substring(0, 160) + '...' : post.excerpt;

	return (
		<PageWrapper>
			<Head>
				<title>{`${post.title} - Spacedrive Blog`}</title>
				<meta name="description" content={description} />
				<meta property="og:title" content={post.title} />
				<meta property="og:description" content={description} />
				<meta property="og:image" content={post.image} />
				<meta content="summary_large_image" name="twitter:card" />
				<meta name="author" content={post.author} />
			</Head>
			<div className="lg:prose-xs prose dark:prose-invert container m-auto mb-20 max-w-4xl p-4 pt-14">
				<>
					<figure>
						<Image
							src={post.image}
							alt={post.imageAlt ?? ''}
							className="mt-8 rounded-xl"
							height={400}
							width={900}
						/>
					</figure>
					<section className="-mx-8 flex flex-wrap gap-4 rounded-xl px-8">
						<div className="w-full grow">
							<h1 className="m-0 text-2xl leading-snug sm:text-4xl sm:leading-normal">
								{post.title}
							</h1>
							<p className="m-0 mt-2">
								by <b>{post.author}</b> &middot;{' '}
								{new Date(post.date).toLocaleDateString()}
							</p>
						</div>
						<div className="flex flex-wrap gap-2">
							{post.tags.map((tag) => (
								<BlogTag key={tag} name={tag} />
							))}
						</div>
					</section>
					<article id="content" className="text-lg">
						<MDXContent components={BlogMDXComponents} />
					</article>
				</>
			</div>
		</PageWrapper>
	);
}
