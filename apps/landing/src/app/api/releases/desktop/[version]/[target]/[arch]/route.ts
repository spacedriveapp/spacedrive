import { redirect } from 'next/navigation';
import { z } from 'zod';
import { getLatestRelease, getRecentReleases, getRelease, githubFetch } from '~/app/api/github';

const version = z.union([z.literal('stable'), z.literal('alpha')]);
const tauriTarget = z.union([z.literal('linux'), z.literal('windows'), z.literal('darwin')]);
const tauriArch = z.union([z.literal('x86_64'), z.literal('aarch64')]);

const extensions = {
	linux: 'deb',
	windows: 'msi',
	darwin: 'dmg'
} as const satisfies Record<z.infer<typeof tauriTarget>, string>;

const paramsSchema = z.object({
	target: tauriTarget,
	arch: tauriArch,
	version: version.or(z.string())
});

export const runtime = 'edge';

export async function GET(
	_: Request,
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
	const params = await paramsSchema.parseAsync(rawParams);

	const release = await (async () => {
		switch (params.version) {
			case 'alpha': {
				const data = await githubFetch(getRecentReleases);

				return data.find((d: any) => d.tag_name.includes('alpha'));
			}
			case 'stable':
				return await githubFetch(getLatestRelease);
			default:
				return await githubFetch(getRelease(params.version));
		}
	})();

	if (!release) return Response.json({ error: 'Release not found' }, { status: 404 });

	params.version = release.tag_name;

	const name = `Spacedrive-${params.target}-${params.arch}.${extensions[params.target]}` as const;

	const asset = release.assets?.find((asset: any) => asset.name === name);

	if (!asset) return Response.json({ error: 'Asset not found' }, { status: 404 });

	return redirect(asset.browser_download_url);
}
