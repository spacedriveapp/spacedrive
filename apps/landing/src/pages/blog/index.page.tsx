import { PostOrPage, PostsOrPages, Tag } from '@tryghost/content-api';
import React, { useEffect, useState } from 'react';
import { Helmet } from 'react-helmet';

import { BlogTag } from '../../components/BlogTag';
import { getWindow } from '../../utils';
import { blogEnabled } from './api';

function Page({ posts }: { posts: PostOrPage[] }) {
	if (!blogEnabled) {
		let window = getWindow();
		if (!window) return;
		window.location.href = '/blog-not-enabled';
		return <></>;
	}

	return (
		<div className="container flex flex-col max-w-4xl gap-20 p-4 m-auto mt-32 mb-20 prose lg:prose-xs dark:prose-invert">
			<Helmet>
				<title>Spacedrive Blog</title>
				<meta name="description" content="Get the latest from Spacedrive." />
			</Helmet>
			<section>
				<h1 className="m-0 fade-in-heading">Blog</h1>
				<p className="fade-in-heading animation-delay-1">Get the latest from Spacedrive.</p>
			</section>
			<section className="grid grid-cols-1 gap-4 sm:grid-cols-1 lg:grid-cols-1 fade-in will-change-transform animation-delay-2">
				{posts.map((post) => {
					return (
						<div
							onClick={() => {
								window.location.href = `/blog/${post.slug}`;
							}}
							className="relative z-0 flex flex-col gap-2 mb-8 overflow-hidden transition-colors border border-gray-500 cursor-pointer rounded-xl"
						>
							{post.feature_image && (
								<img
									src={post.feature_image}
									alt=""
									className="inset-0 object-cover w-full m-0 md:h-96 -z-10 rounded-t-xl"
								/>
							)}
							<div className="p-8">
								<h2 className="m-0 text2xl md:text-4xl">{post.title}</h2>
								<small className="m-0">{post.reading_time} minute read.</small>
								<p className="my-2 line-clamp-3">{post.excerpt}</p>
								<p className="m-0 text-white">
									by {post.primary_author?.name} &middot;{' '}
									{new Date(post.published_at ?? '').toLocaleDateString()}
								</p>
								<div className="flex flex-wrap gap-2 mt-4">
									{post.tags?.map((tag: Tag) => {
										return <BlogTag tag={tag} />;
									})}
								</div>
							</div>
						</div>
					);
				})}
			</section>
		</div>
	);
}

export { Page };
