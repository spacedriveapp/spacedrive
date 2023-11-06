import { useQuery } from '@tanstack/react-query';
import Markdown from 'react-markdown';
import { usePlatform } from '~/util/Platform';

import { Heading } from '../Layout';

export const Component = () => {
	const platform = usePlatform();

	const changelog = useQuery(['changelog'], () =>
		fetch(`${platform.landingApiOrigin}/api/releases`).then((r) => r.json())
	);

	return (
		<>
			<Heading title="Changelog" description="See what cool new features we're making" />
			{changelog.data?.map((release: any) => (
				<article key={release.version} className={'prose prose-sm prose-invert'}>
					<Markdown
						className=""
						skipHtml
						components={{
							a(props) {
								let href = props.href!;

								if (href.startsWith('../../')) {
									href = `${platform.landingApiOrigin}/docs/${href.replace(
										'../../',
										''
									)}`;
								}
								return (
									<a
										{...props}
										href={href}
										onClick={(e) => {
											e.preventDefault();

											platform.openLink(href);
										}}
									/>
								);
							}
						}}
					>{`# ${release.version}\n${release.body}`}</Markdown>
				</article>
			))}
		</>
	);
};
