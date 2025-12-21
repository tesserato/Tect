import * as path from 'path';
import { workspace, ExtensionContext } from 'vscode';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    TransportKind
} from 'vscode-languageclient/node';

let client: LanguageClient;

export function activate(context: ExtensionContext) {
    // 1. Path to your Rust binary
    // During development, it points to your target/debug folder.
    // In production, you would package the binary with the extension.
    const serverModule = context.asAbsolutePath(
        path.join('..', '..', 'target', 'debug', 'tect') // Remove .exe on Linux/Mac
    );

    // 2. Server Options (How to launch the Rust process)
    const serverOptions: ServerOptions = {
        run: { command: serverModule, transport: TransportKind.stdio },
        debug: { command: serverModule, transport: TransportKind.stdio }
    };

    // 3. Client Options (What files to watch)
    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: 'file', language: 'tect' }],
        synchronize: {
            // Notify the server about file changes in the workspace
            fileEvents: workspace.createFileSystemWatcher('**/*.tect')
        }
    };

    // 4. Create and start the client
    client = new LanguageClient(
        'tectServer',
        'Tect Language Server',
        serverOptions,
        clientOptions
    );

    console.log('Tect extension is now active.');
    client.start();
}

export function deactivate(): Thenable<void> | undefined {
    if (!client) {
        return undefined;
    }
    return client.stop();
}