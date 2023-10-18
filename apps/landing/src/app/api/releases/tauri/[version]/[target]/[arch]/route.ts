import { NextResponse } from 'next/server';
import { z } from 'zod';
import { getLatestRelease, getRecentReleases, getRelease, githubFetch } from '~/app/api/github';
import { env } from '~/env';

const version = z.union([z.literal('stable'), z.literal('alpha')]);
const tauriTarget = z.union([z.literal('linux'), z.literal('windows'), z.literal('darwin')]);
const tauriArch = z.union([z.literal('x86_64'), z.literal('aarch64')]);

const paramsSchema = z.object({
	target: tauriTarget,
	arch: tauriArch,
	version: version.or(z.string())
});

type TauriResponse = {
	// Must be > than the version in tauri.conf.json for update to be detected
	version: string;
	pub_date: string;
	url: string;
	signature: string;
	notes: string;
};

export const runtime = 'edge';

export async function GET(
	req: Request,
	{
		params: rawParams
	}: {
		params: {
			version: string;
			target: string;
			arch: string;
		};
	}
) {
	const params = await paramsSchema.parseAsync({
		...rawParams,
		// prefer header override to support release channels
		version: req.headers.get('X-Spacedrive-Version') ?? rawParams.version
	});

	const release = await fetchRelease(params);

	if (!release || !release.published_at)
		return NextResponse.json({ error: 'Release not found' }, { status: 404 });

	params.version = release.tag_name;

	const asset = release.assets.find(({ name }: any) => name === binaryName(params));

	if (!asset) return NextResponse.json({ error: 'Asset not found' }, { status: 404 });

	const signatureAsset = release.assets.find(
		({ name }: any) => name === `${binaryName(params)}.sig`
	);

	if (!signatureAsset)
		return NextResponse.json({ error: 'Signature asset not found' }, { status: 404 });

	const signature = await fetch(signatureAsset.browser_download_url).then((r) => r.text());

	const response: TauriResponse = {
		version: release.tag_name,
		url: asset.browser_download_url,
		signature,
		notes: release.body ?? '',
		pub_date: release.published_at!
	};

	return Response.json(response);
}

async function fetchRelease({ version }: z.infer<typeof paramsSchema>) {
	switch (version) {
		case 'alpha': {
			const data = await githubFetch(getRecentReleases);

			return data.find((d: any) => d.tag_name.includes('alpha'));
		}
		case 'stable':
			return githubFetch(getLatestRelease);
		default:
			return githubFetch(getRelease(version));
	}
}

function binaryName({ target, arch }: z.infer<typeof paramsSchema>) {
	const ext = extensionForTarget(target);

	return `Spacedrive-Updater-${target}-${arch}.${ext}`;
}

function extensionForTarget(target: z.infer<typeof tauriTarget>) {
	if (target === 'windows') return 'zip';
	else return 'tar.gz';
}
