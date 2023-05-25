import { getServerSession } from '../../[...auth]/auth';

export async function GET(req: Request) {
	const session = await getServerSession(req);

	// TODO: Get from Drizzle

	return new Response(JSON.stringify(session));
}
