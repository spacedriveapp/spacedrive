import { PostOrPage, PostsOrPages, Tag } from '@tryghost/content-api';
import React, { useEffect, useState } from 'react';

import { blogEnabled, getPosts } from './posts';

function Page() {
	if (!blogEnabled) {
		window.location.href = '/';
		return <></>;
	}

	const [posts, setPosts] = useState<PostsOrPages | never[]>([]);

	useEffect(() => {
		const get = async () => {
			const posts: PostsOrPages | never[] = await getPosts();
			setPosts(posts);
		};
		get();
	}, []);

	console.log(posts);

	return (
		<div className="container max-w-4xl p-4 mt-32 mb-20 m-auto prose lg:prose-xs dark:prose-invert flex flex-col gap-20">
			<section>
				<h1 className="m-0">Blog</h1>
				<p className="m-0">Get the latest from Spacedrive.</p>
			</section>
			<section className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
				{posts.map((post: PostOrPage) => {
					return (
						<div
							onClick={() => {
								window.location.href = `/blog/${post.slug}`;
							}}
							className="cursor-pointer transition-colors relative p-8 rounded-xl flex flex-col gap-2 
							bg-gray-850/80 hover:bg-gray-850/100 z-0 overflow-hidden"
						>
							{post.feature_image && (
								<img
									src={post.feature_image}
									alt=""
									className="-z-10 blur-md absolute inset-0 object-cover m-0 rounded-xl h-full w-full opacity-10"
								/>
							)}
							<h2 className="m-0">{post.title}</h2>
							<small className="m-0">{post.reading_time} minute read.</small>
							<p className="my-2 line-clamp-3">{post.excerpt}</p>
							<p className="m-0 text-white">
								by {post.primary_author?.name} &middot;{' '}
								{new Date(post.published_at ?? '').toLocaleDateString()}
							</p>
							<div className="flex flex-wrap gap-2">
								{post.tags?.map((tag: Tag) => {
									return (
										<span
											className={`px-2 py-1 rounded-full text-sm`}
											style={{
												backgroundColor: tag.accent_color + '' ?? '',
												color:
													parseInt(tag.accent_color?.slice(1) ?? '', 16) > 0xffffff / 2
														? '#000'
														: '#fff'
											}}
										>
											{tag.name}
										</span>
									);
								})}
							</div>
						</div>
					);
				})}
			</section>
		</div>
	);
}

export default Page;
