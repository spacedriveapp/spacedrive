import { Helmet } from 'react-helmet';
import { BlogTag } from '../../components/BlogTag';
import { BlogPosts } from './api';

function Page({ posts }: { posts: BlogPosts }) {
	const postsArray = Object.values(posts);
	return (
		<div className="lg:prose-xs prose dark:prose-invert container m-auto mb-20 flex max-w-4xl flex-col gap-20 p-4 pt-32">
			<Helmet>
				<title>Spacedrive Blog</title>
				<meta name="description" content="Get the latest from Spacedrive." />
			</Helmet>
			<section>
				<h1 className="fade-in-heading m-0">Blog</h1>
				<p className="fade-in-heading animation-delay-1">Get the latest from Spacedrive.</p>
			</section>
			<section className="animation-delay-2 grid grid-cols-1 gap-4 will-change-transform fade-in sm:grid-cols-1 lg:grid-cols-1">
				{postsArray.map((post) => (
					<a
						key={post.slug}
						href={`/blog/${post.slug}`}
						className="relative z-0 mb-8 flex cursor-pointer flex-col gap-2 overflow-hidden rounded-xl border border-gray-500 transition-colors"
					>
						{post.image && (
							<img
								src={`/${post.image}`}
								alt=""
								className="inset-0 -z-10 m-0 w-full rounded-t-xl object-cover md:h-96"
							/>
						)}
						<div className="p-8">
							<h2 className="text2xl m-0 md:text-4xl">{post.title}</h2>
							<small className="m-0">{post.readTime} minute read.</small>
							{/* <p className="line-clamp-3 my-2">{post.excerpt}</p> */}
							<p className="m-0 text-white">
								by {post.author} &middot;{' '}
								{new Date(post.date ?? '').toLocaleDateString()}
							</p>
							<div className="mt-4 flex flex-wrap gap-2">
								{post.tags?.map((tag) => (
									<BlogTag key={tag} name={tag} />
								))}
							</div>
						</div>
					</a>
				))}
			</section>
		</div>
	);
}

export { Page };
