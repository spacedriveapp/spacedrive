import { SpacedriveClient, SpacedriveError } from '../src';

describe('SpacedriveClient', () => {
    let client: SpacedriveClient;

    beforeEach(() => {
        client = new SpacedriveClient('/tmp/test.sock');
    });

    test('should initialize correctly', () => {
        expect(client).toBeInstanceOf(SpacedriveClient);
    });

    test('should create error types correctly', () => {
        const error = SpacedriveError.connectionFailed('Test error');
        expect(error).toBeInstanceOf(SpacedriveError);
        expect(error.message).toBe('Test error');
        expect(error.type).toBe('connection');
    });

    // More tests will be added once the daemon connection is implemented
});
