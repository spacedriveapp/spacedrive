import {docsContentRoute} from '@/lib/shared';
import {isMarkdownPreferred} from 'fumadocs-core/negotiation';
import {NextRequest, NextResponse} from 'next/server';

const passthroughPrefixes = ['/llms', '/api/', '/og/', '/_next'];

export default function proxy(request: NextRequest) {
	const {pathname} = request.nextUrl;
	if (passthroughPrefixes.some((prefix) => pathname.startsWith(prefix))) {
		return NextResponse.next();
	}

	// Docs pages are served from the site root, so `/core/jobs.md` and
	// markdown-preferring clients both rewrite to the markdown route.
	if (pathname.endsWith('.md')) {
		const path = pathname.slice(0, -'.md'.length);
		return NextResponse.rewrite(
			new URL(`${docsContentRoute}${path}/content.md`, request.nextUrl)
		);
	}

	if (isMarkdownPreferred(request)) {
		const path = pathname === '/' ? '' : pathname;
		return NextResponse.rewrite(
			new URL(`${docsContentRoute}${path}/content.md`, request.nextUrl),
			{headers: {Vary: 'Accept'}}
		);
	}

	return NextResponse.next();
}
