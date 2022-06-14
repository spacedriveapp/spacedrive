import compression from 'compression';
import express from 'express';
import { createPageRenderer } from 'vite-plugin-ssr';

const isProduction = process.env.NODE_ENV === 'production';
const root = `${__dirname}/..`;

startServer();

async function startServer() {
	const app = express();

	app.use(compression());

	let viteDevServer;
	if (isProduction) {
		app.use(express.static(`${root}/dist/client`));
	} else {
		const vite = require('vite');
		viteDevServer = await vite.createServer({
			root,
			server: { middlewareMode: 'ssr' }
		});
		app.use(viteDevServer.middlewares);
	}

	const renderPage = createPageRenderer({ viteDevServer, isProduction, root });
	app.get('*', async (req, res, next) => {
		const url = req.originalUrl;
		const pageContextInit = {
			url
		};
		const pageContext = await renderPage(pageContextInit);
		const { httpResponse } = pageContext;
		if (!httpResponse) return next();
		const { body, statusCode, contentType } = httpResponse;
		res.status(statusCode).type(contentType).send(body);
	});

	const port = process.env.PORT || 3000;
	app.listen(port);
	console.log(`Server running at http://localhost:${port}`);
}
