import { defineEnvVars } from '@sveltejs/kit/env';

const DEFAULT_GATEWAY_URL = 'https://localhost:8000';

export const variables = defineEnvVars({
    API_GATEWAY_URL: {
        schema: (value) => value ?? DEFAULT_GATEWAY_URL,
        description: 'Base URL of the api gateway used to validate sessions on the server'
    }
});
