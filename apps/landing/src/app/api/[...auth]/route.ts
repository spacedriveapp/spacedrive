import { Auth } from '@auth/core';
import { authOptions } from './auth';

export const runtime = 'edge';

const handler = (req: Request) => Auth(req, authOptions);

export const GET = handler;
export const POST = handler;
