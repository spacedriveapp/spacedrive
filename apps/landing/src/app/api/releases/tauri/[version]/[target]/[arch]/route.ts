import { NextResponse } from 'next/server';
import { z } from 'zod';
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

	const release = await getRelease(params);

	if (!release) return NextResponse.json({ error: 'Release not found' }, { status: 404 });

	params.version = release.tag_name;

	const asset = release.assets.find(({ name }: any) => name === binaryName(params));

	if (!asset) {
		console.error('Failed to find asset', asset);
		return NextResponse.json({ error: 'Asset not found' }, { status: 404 });
	}

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
		notes: release.body,
		pub_date: release.published_at
	};

	return NextResponse.json(response);
}

async function getRelease({ version }: z.infer<typeof paramsSchema>): Promise<any> {
	switch (version) {
		case 'alpha': {
			const data = await githubFetch(`/repos/${env.GITHUB_ORG}/${env.GITHUB_REPO}/releases`);

			return data.find((d: any) => d.tag_name.includes('alpha'));
		}
		case 'stable':
			return githubFetch(`/repos/${env.GITHUB_ORG}/${env.GITHUB_REPO}/releases/latest`);
		default:
			return githubFetch(
				`/repos/$${env.GITHUB_ORG}/${env.GITHUB_REPO}/releases/tags/${version}`
			);
	}
}

const FETCH_META = {
	headers: {
		Authorization: `Bearer ${env.GITHUB_PAT}`,
		Accept: 'application/vnd.github+json'
	},
	next: {
		revalidate: 60
	}
} as RequestInit;

async function githubFetch(path: string) {
	return fetch(`https://api.github.com${path}`, FETCH_META).then((r) => r.json());
}

function binaryName({ target, arch }: z.infer<typeof paramsSchema>) {
	const ext = extensionForTarget(target);

	return `Spacedrive-Updater-${target}-${arch}.${ext}`;
}

function extensionForTarget(target: z.infer<typeof tauriTarget>) {
	if (target === 'windows') return 'zip';
	else return 'tar.gz';
}
