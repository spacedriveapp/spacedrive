import { PostOrPage, Tag } from '@tryghost/content-api';
import Prism from 'prismjs';
import 'prismjs/components/prism-rust';
import React, { useEffect, useState } from 'react';
import { Helmet } from 'react-helmet';

import '../../atom-one.css';
import { BlogTag } from '../../components/BlogTag';

function MarkdownPage({ post }: { post: PostOrPage }) {
	useEffect(() => {
		Prism.highlightAll();
	}, []);

	console.log(post);

	return (
		<>
			<Helmet>
				<title>{post?.title} - Spacedrive Blog</title>
				<meta name="description" content={post?.excerpt} />
				<meta property="og:title" content={post?.title} />
				<meta property="og:description" content={post?.excerpt} />
				<meta property="og:image" content={post?.feature_image as string} />
				<meta name="author" content={post?.primary_author?.name || 'Spacedrive Technology Inc.'} />
			</Helmet>
			<div className="container max-w-4xl p-4 m-auto mt-8 mb-20 prose lg:prose-xs dark:prose-invert">
				{post && (
					<>
						<figure>
							<figcaption
								dangerouslySetInnerHTML={{ __html: post.feature_image_caption as any }}
							></figcaption>
							<img src={post?.feature_image as string} alt="" className="rounded-xl" />
						</figure>
						<section className="flex flex-wrap gap-4 px-8 -mx-8 rounded-xl">
							<div className="flex-grow">
								<h1 className="m-0 text-2xl leading-snug sm:leading-normal sm:text-4xl">
									{post?.title}
								</h1>
								<p className="m-0 mt-2">
									by <b>{post?.primary_author?.name}</b> &middot;{' '}
									{new Date(post?.published_at ?? '').toLocaleDateString()}
								</p>
							</div>
							<div className="flex flex-wrap gap-2">
								{post?.tags?.map((tag: Tag) => {
									return <BlogTag tag={tag} />;
								})}
							</div>
						</section>
						<article
							id="content"
							className="text-lg"
							dangerouslySetInnerHTML={{ __html: post.html as string }}
						></article>
					</>
				)}
			</div>
		</>
	);
}

export default MarkdownPage;
