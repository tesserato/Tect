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

/**
 * Entry point for the VS Code extension. 
 * Orchestrates binary discovery and Language Client lifecycle.
 */
export function activate(context: ExtensionContext) {
    // Platform-specific binary discovery (LSP is written in Rust)
    const isWindows = os.platform() === 'win32';
    const binaryName = isWindows ? 'tect.exe' : 'tect';

    // Discovery logic: Looks in the target/debug folder relative to the extension
    const serverModule = context.asAbsolutePath(
        path.join('..', '..', 'target', 'debug', binaryName)
    );

    console.log(`Tect: Starting Language Server from: ${serverModule}`);

    // Configuration for launching the server process via stdio.
    // Explicitly passing 'serve' to match the Rust CLI subcommand.
    const serverOptions: ServerOptions = {
        run: {
            command: serverModule,
            args: ['serve'],
            transport: TransportKind.stdio
        },
        debug: {
            command: serverModule,
            args: ['serve'],
            transport: TransportKind.stdio
        }
    };

    // Client-side configuration: watch .tect files and handle synchronization
    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: 'file', language: 'tect' }],
        synchronize: {
            fileEvents: workspace.createFileSystemWatcher('**/*.tect')
        }
    };

    // Instantiate and launch the Language Client
    client = new LanguageClient(
        'tectServer',
        'Tect Language Server',
        serverOptions,
        clientOptions
    );

    // Start the client and log connectivity status
    client.start().then(() => {
        console.log('Tect: Language Server connected successfully.');
    }).catch((err) => {
        console.error('Tect: Failed to establish server connection:', err);
    });
}

/**
 * Cleanly terminates the Language Client connection upon extension deactivation.
 */
export function deactivate(): Thenable<void> | undefined {
    if (!client) {
        return undefined;
    }
    return client.stop();
}