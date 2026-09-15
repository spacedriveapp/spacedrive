import {appName, getPageImageUrl} from '@/lib/shared';
import {source} from '@/lib/source';
import {generateOGImage} from 'fumadocs-ui/og';
import {notFound} from 'next/navigation';

export const revalidate = false;

export async function GET(
	_req: Request,
	{params}: RouteContext<'/og/docs/[...slug]'>
) {
	const {slug} = await params;
	const page = source.getPage(slug.slice(0, -1));
	if (!page) notFound();

	return generateOGImage({
		title: page.data.title,
		description: page.data.description,
		site: appName
	});
}

export function generateStaticParams() {
	return source.getPages().map((page) => ({
		lang: page.locale,
		slug: getPageImageUrl(page).segments
	}));
}
