import { allDocs } from '@contentlayer/generated';

export function getStaticProps() {
	return { props: { docs: allDocs } };
}
