import * as path from 'path';
import * as os from 'os';
import { workspace, ExtensionContext } from 'vscode';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    TransportKind
} from 'vscode-languageclient/node';

let client: LanguageClient;

export function activate(context: ExtensionContext) {
    // Determine binary name based on OS
    const isWindows = os.platform() === 'win32';
    const binaryName = isWindows ? 'tect.exe' : 'tect';

    // Path to the Rust binary in the workspace root target folder
    // This assumes the extension is at <root>/editors/vscode
    const serverModule = context.asAbsolutePath(
        path.join('..', '..', 'target', 'debug', binaryName)
    );

    console.log(`Attempting to start Tect LS from: ${serverModule}`);

    const serverOptions: ServerOptions = {
        run: { command: serverModule, transport: TransportKind.stdio },
        debug: { command: serverModule, transport: TransportKind.stdio }
    };

    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: 'file', language: 'tect' }],
        synchronize: {
            fileEvents: workspace.createFileSystemWatcher('**/*.tect')
        }
    };

    client = new LanguageClient(
        'tectServer',
        'Tect Language Server',
        serverOptions,
        clientOptions
    );

    client.start().then(() => {
        console.log('Tect Language Server started successfully.');
    }).catch((err) => {
        console.error('Failed to start Tect Language Server:', err);
    });
}

export function deactivate(): Thenable<void> | undefined {
    if (!client) {
        return undefined;
    }
    return client.stop();
}