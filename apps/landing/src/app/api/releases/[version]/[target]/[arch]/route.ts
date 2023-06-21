import { NextResponse } from 'next/server';
import { z } from 'zod';
import { env } from '~/env';
import * as schemas from './schemas';
import { TauriResponse } from './schemas';

export const runtime = 'edge';

const ORG = 'brendonovich';
const REPO = 'spacedrive';

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

async function getRelease({ version }: z.infer<typeof schemas.params>): Promise<any> {
	switch (version) {
		case 'alpha':
			const data = await githubFetch(`/repos/${ORG}/${REPO}/releases`);

			return data.find((d: any) => d.tag_name.includes('alpha'));
		case 'stable':
			return githubFetch(`https://api.github.com/repos/${ORG}/${REPO}/releases/latest`);
		default:
			return githubFetch(
				`https://api.github.com/repos/${ORG}/${REPO}/releases/tags/${version}`
			);
	}
}

export async function GET(_: Request, extra: { params: object }) {
	let params = await schemas.params.parseAsync(extra.params);

	const release = await getRelease(params);

	if (!release) return NextResponse.json({ error: 'Release not found' }, { status: 404 });

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
		version: params.version,
		url: asset.browser_download_url,
		signature,
		notes: '',
		pub_date: asset.created_at
	};

	return NextResponse.json(response);
}

const extensionForTarget = (target: z.infer<typeof schemas.tauriTarget>) => {
	if (target === 'windows') return 'zip';
	else return 'tar.gz';
};

const binaryName = ({ version, target, arch }: z.infer<typeof schemas.params>) => {
	const ext = extensionForTarget(target);

	return `Spacedrive-Updater-${version}-${target}-${arch}.${ext}`;
};
