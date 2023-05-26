import { getServerSession } from '../../[...auth]/auth';

export async function GET(req: Request) {
	return new Response(JSON.stringify(await getServerSession(req)), {
		headers: {
			'content-type': 'application/json'
		}
	});
}
