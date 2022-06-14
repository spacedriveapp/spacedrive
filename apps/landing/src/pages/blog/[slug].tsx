import { PostOrPage, Tag } from '@tryghost/content-api';
import Prism from 'prismjs';
import 'prismjs/components/prism-rust';
import React, { useEffect, useState } from 'react';

import '../../atom-one.css';
import { getPost } from './posts';

function MarkdownPage() {
	const [post, setPost] = useState<PostOrPage | null>(null);

	useEffect(() => {
		const get = async () => {
			const post = await getPost(window.location.pathname.split('/blog/')[1]);
			setPost(post);
		};
		get();
		Prism.highlightAll();
	}, []);
	return (
		<div className="container max-w-4xl p-4 m-auto mt-32 mb-20 prose lg:prose-xs dark:prose-invert">
			<figure>
				{/* @ts-expect-error */}
				<figcaption dangerouslySetInnerHTML={{ __html: post.feature_image_caption }}></figcaption>
				<img src={post?.feature_image as string} alt="" className="rounded-xl" />
			</figure>
			<section className="flex flex-wrap gap-4 p-8 -mx-8 rounded-xl">
				<div className="flex-grow">
					<h1 className="m-0">{post?.title}</h1>
					<p className="m-0">
						by {post?.primary_author?.name} &middot;{' '}
						{new Date(post?.published_at ?? '').toLocaleDateString()}
					</p>
				</div>
				<div className="flex flex-wrap gap-2">
					{post?.tags?.map((tag: Tag) => {
						return (
							<span
								className={`px-2 py-1 rounded-full text-sm h-8`}
								style={{
									backgroundColor: tag.accent_color + '' ?? '',
									color:
										parseInt(tag.accent_color?.slice(1) ?? '', 16) > 0xffffff / 2 ? '#000' : '#fff'
								}}
							>
								{tag.name}
							</span>
						);
					})}
				</div>
			</section>
			{/* @ts-expect-error */}
			<article id="content" dangerouslySetInnerHTML={{ __html: post.html }}></article>
		</div>
	);
}

export default MarkdownPage;
