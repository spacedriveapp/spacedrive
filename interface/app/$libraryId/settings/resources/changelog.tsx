import { useQuery } from '@tanstack/react-query';
import clsx from 'clsx';
import Markdown from 'react-markdown';
import { useIsDark, useLocale } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import { Heading } from '../Layout';

export const Component = () => {
	const platform = usePlatform();
	const isDark = useIsDark();
	const changelog = useQuery({
		queryKey: ['changelog'],
		queryFn: () => fetch(`${platform.landingApiOrigin}/api/releases`).then((r) => r.json())
	});

	const { t } = useLocale();

	return (
		<>
			<Heading
				title={t('changelog_page_title')}
				description={t('changelog_page_description')}
			/>
			{changelog.data?.map((release: any) => (
				<article
					key={release.version}
					className={clsx(
						'prose prose-sm text-ink prose-headings:font-plex',
						isDark && 'prose-invert'
					)}
				>
					<Markdown
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
