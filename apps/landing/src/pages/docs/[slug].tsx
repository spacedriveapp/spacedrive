import { allDocs } from '@contentlayer/generated';

export async function getStaticPaths() {
	const paths = allDocs.map((doc) => doc.url);
	return {
		paths,
		fallback: false
	};
}

export async function getStaticProps({ params }: { params: { slug: string } }) {
	const doc = allDocs.find((doc) => doc.slug === params.slug);

	if (!doc) {
		return {
			notFound: true
		};
	}

	return {
		props: {
			doc
		}
	};
}
