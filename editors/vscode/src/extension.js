"use strict";
exports.__esModule = true;
exports.deactivate = exports.activate = void 0;
var path = require("path");
var vscode_1 = require("vscode");
var node_1 = require("vscode-languageclient/node");
var client;
function activate(context) {
    // 1. Path to your Rust binary
    // During development, it points to your target/debug folder.
    // In production, you would package the binary with the extension.
    var serverModule = context.asAbsolutePath(path.join('..', '..', 'target', 'debug', 'tect') // Remove .exe on Linux/Mac
    );
    // 2. Server Options (How to launch the Rust process)
    var serverOptions = {
        run: { command: serverModule, transport: node_1.TransportKind.stdio },
        debug: { command: serverModule, transport: node_1.TransportKind.stdio }
    };
    // 3. Client Options (What files to watch)
    var clientOptions = {
        documentSelector: [{ scheme: 'file', language: 'tect' }],
        synchronize: {
            // Notify the server about file changes in the workspace
            fileEvents: vscode_1.workspace.createFileSystemWatcher('**/*.tect')
        }
    };
    // 4. Create and start the client
    client = new node_1.LanguageClient('tectServer', 'Tect Language Server', serverOptions, clientOptions);
    console.log('Tect extension is now active.');
    client.start();
}
exports.activate = activate;
function deactivate() {
    if (!client) {
        return undefined;
    }
    return client.stop();
}
exports.deactivate = deactivate;
