import fastify from 'fastify';
import { postgraphile } from 'postgraphile';
import dotenv from 'dotenv';

dotenv.config();

const PORT = parseInt(process.env.PORT || '3000', 10);
const DATABASE_URL = process.env.DATABASE_URL || '';

const app = fastify();

// PostGraphile middleware
app.register(async (instance) => {
    instance.addHook('onRequest', (req, res, done) => {
        done();
    });

    const pgMiddleware = postgraphile(DATABASE_URL, 'public', {
        graphiql: true,
        enhanceGraphiql: true,
        enableCors: true,
    });

    instance.route({
        method: ['GET', 'POST'],
        url: '/graphql',
        handler: async (req, res) => {
            // @ts-ignore
            await pgMiddleware(req.raw, res.raw);
        },
    });

    instance.route({
        method: 'GET',
        url: '/graphiql',
        handler: async (req, res) => {
            // @ts-ignore
            await pgMiddleware(req.raw, res.raw);
        },
    });
});

// Health check
app.get('/health', async () => {
    return { status: 'healthy' };
});

export async function startServer() {
    try {
        await app.listen({ port: PORT, host: '0.0.0.0' });
        console.log(`Server running at http://localhost:${PORT}`);
        console.log(`GraphiQL available at http://localhost:${PORT}/graphiql`);
    } catch (err) {
        app.log.error(err);
        process.exit(1);
    }
}
