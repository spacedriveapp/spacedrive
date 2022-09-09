import { PostOrPage } from '@tryghost/content-api';
import { Helmet } from 'react-helmet';

function Page({ posts }: { posts: PostOrPage[] }) {
	return (
		<div className="container flex flex-col max-w-4xl gap-20 p-4 m-auto mt-32 mb-20 prose lg:prose-xs dark:prose-invert">
			<Helmet>
				<title>Spacedrive Docs</title>
				<meta name="description" content="Learn more about the Explorer" />
			</Helmet>

			<section className="grid grid-cols-1 gap-4 sm:grid-cols-1 lg:grid-cols-1 fade-in will-change-transform animation-delay-2">
				Jeff
			</section>
		</div>
	);
}

export { Page };
