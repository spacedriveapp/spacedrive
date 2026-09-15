import {getMDXComponents} from '@/components/mdx';
import {getPageImageUrl, getPageMarkdownUrl, gitConfig} from '@/lib/shared';
import {source} from '@/lib/source';
import {
	DocsBody,
	DocsDescription,
	DocsPage,
	DocsTitle,
	MarkdownCopyButton,
	ViewOptionsPopover
} from 'fumadocs-ui/layouts/docs/page';
import {createRelativeLink} from 'fumadocs-ui/mdx';
import type {Metadata} from 'next';
import {notFound, redirect} from 'next/navigation';

export default async function Page(props: PageProps<'/[[...slug]]'>) {
	const params = await props.params;
	if (!params.slug?.length) redirect('/overview/introduction');

	const page = source.getPage(params.slug);
	if (!page) notFound();

	const MDX = page.data.body;
	const markdownUrl = getPageMarkdownUrl(page).url;

	return (
		<DocsPage toc={page.data.toc} full={page.data.full}>
			<DocsTitle>{page.data.title}</DocsTitle>
			<DocsDescription className="mb-0">
				{page.data.description}
			</DocsDescription>
			<div className="flex flex-row items-center gap-2 border-b pb-6">
				<MarkdownCopyButton markdownUrl={markdownUrl} />
				<ViewOptionsPopover
					markdownUrl={markdownUrl}
					githubUrl={`https://github.com/${gitConfig.user}/${gitConfig.repo}/blob/${gitConfig.branch}/docs/${page.path}`}
				/>
			</div>
			<DocsBody>
				<MDX
					components={getMDXComponents({
						a: createRelativeLink(source, page)
					})}
				/>
			</DocsBody>
		</DocsPage>
	);
}

export async function generateStaticParams() {
	return source.generateParams();
}

export async function generateMetadata(
	props: PageProps<'/[[...slug]]'>
): Promise<Metadata> {
	const params = await props.params;
	if (!params.slug?.length) return {};

	const page = source.getPage(params.slug);
	if (!page) notFound();

	return {
		title: page.data.title,
		description: page.data.description,
		openGraph: {
			images: getPageImageUrl(page).url
		}
	};
}
