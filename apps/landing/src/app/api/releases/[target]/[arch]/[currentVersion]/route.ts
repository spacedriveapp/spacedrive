import { get } from '@vercel/edge-config';
import { NextResponse } from 'next/server';

export const runtime = 'edge';

type Channel = 'stable' | 'beta';
type TauriTarget = 'linux' | 'windows' | 'darwin';
type TauriArch = 'x86_64' | 'i686' | 'aarch64' | 'armv7';

export async function GET(
	_: Request,
	{
		params
	}: {
		params: {
			target: TauriTarget;
			arch: TauriArch;
			currentVersion: string;
		};
	}
) {
	const releases = await get<Record<string, any>>('main');

	return NextResponse.json(releases![params.target]);
}
