import { get } from '@vercel/edge-config';
import { NextResponse } from 'next/server';

export const config = {
	runtime: 'edge'
};

export async function GET(
	_: Request,
	{
		params
	}: {
		params: {
			target: string;
			arch: string;
			currentVersion: string;
		};
	}
) {
	const releases = await get<Record<string, any>>('main');

	return NextResponse.json(releases![params.target]);
}
