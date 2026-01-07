import * as path from 'path';
import * as os from 'os';
import * as vscode from 'vscode';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    TransportKind
} from 'vscode-languageclient/node';

let client: LanguageClient;

export function activate(context: vscode.ExtensionContext) {
    context.subscriptions.push(
        vscode.commands.registerCommand('tect.openPreview', () => {
            const editor = vscode.window.activeTextEditor;
            if (editor) {
                TectPreviewPanel.createOrShow(context.extensionUri, editor.document.uri);
            }
        })
    );

    // --- Production Binary Discovery ---
    const platform = os.platform(); // 'win32', 'linux', 'darwin'
    const arch = os.arch();         // 'x64', 'arm64'
    
    let binaryName: string;
    if (platform === 'win32') {
        binaryName = 'tect-x86_64-pc-windows-msvc.exe';
    } else if (platform === 'darwin') {
        binaryName = arch === 'arm64' ? 'tect-aarch64-apple-darwin' : 'tect-x86_64-apple-darwin';
    } else {
        binaryName = 'tect-x86_64-unknown-linux-gnu';
    }

    // During development, fall back to target/debug if bin/ doesn't exist
    let serverModule = context.asAbsolutePath(path.join('bin', binaryName));
    if (process.env.VSCODE_DEBUG_MODE === 'true' || !require('fs').existsSync(serverModule)) {
        const debugExec = platform === 'win32' ? 'tect.exe' : 'tect';
        serverModule = context.asAbsolutePath(path.join('..', '..', 'target', 'debug', debugExec));
    }

    const serverOptions: ServerOptions = {
        run: { command: serverModule, args: ['serve'], transport: TransportKind.stdio },
        debug: { command: serverModule, args: ['serve'], transport: TransportKind.stdio }
    };

    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: 'file', language: 'tect' }],
        synchronize: { fileEvents: vscode.workspace.createFileSystemWatcher('**/*.tect') }
    };

    client = new LanguageClient('tectServer', 'Tect Language Server', serverOptions, clientOptions);

    client.start().then(() => {
        client.onNotification("tect/analysisFinished", (params: { uri: string }) => {
            TectPreviewPanel.updateIfExists(params.uri);
        });
    });
}

export function deactivate(): Thenable<void> | undefined {
    return client ? client.stop() : undefined;
}

class TectPreviewPanel {
    public static currentPanel: TectPreviewPanel | undefined;
    private readonly _panel: vscode.WebviewPanel;
    private _disposables: vscode.Disposable[] = [];

    public static createOrShow(extensionUri: vscode.Uri, uri: vscode.Uri) {
        if (TectPreviewPanel.currentPanel) {
            TectPreviewPanel.currentPanel._panel.reveal(vscode.ViewColumn.Two);
            return;
        }

        const panel = vscode.window.createWebviewPanel(
            'tectPreview',
            'Tect Architecture',
            vscode.ViewColumn.Two,
            { enableScripts: true, retainContextWhenHidden: true }
        );

        TectPreviewPanel.currentPanel = new TectPreviewPanel(panel, extensionUri, uri);
    }

    public static async updateIfExists(uri: string) {
        if (TectPreviewPanel.currentPanel) {
            TectPreviewPanel.currentPanel.update();
        }
    }

    private constructor(panel: vscode.WebviewPanel, extensionUri: vscode.Uri, private _uri: vscode.Uri) {
        this._panel = panel;
        this._panel.onDidDispose(() => this.dispose(), null, this._disposables);
        this._panel.webview.html = this._getHtmlForWebview();
        this.update();
    }

    public async update() {
        if (!client) return;
        try {
            const visData = await client.sendRequest("tect/getGraph", { uri: this._uri.toString() });
            this._panel.webview.postMessage({ command: 'update', data: visData });
        } catch (e) {
            console.error("Failed to fetch graph data", e);
        }
    }

    private _getHtmlForWebview(): string {
        return `
            <!DOCTYPE html>
            <html style="color-scheme: dark;">
            <head>
                <meta charset="utf-8">
                <script type="text/javascript" src="https://unpkg.com/vis-network/standalone/umd/vis-network.min.js"></script>
                <style>
                    body { background-color: #0b0e14; color: #e0e0e0; margin: 0; padding: 0; overflow: hidden; height: 100vh; font-family: sans-serif; }
                    #mynetwork { width: 100%; height: 100vh; }
                </style>
            </head>
            <body>
                <div id="mynetwork"></div>
                <script>
                    const container = document.getElementById('mynetwork');
                    let network = null;
                    let nodes = new vis.DataSet([]);
                    let edges = new vis.DataSet([]);

                    window.addEventListener('message', event => {
                        const message = event.data;
                        if (message.command === 'update' && message.data) {
                            const data = message.data;
                            
                            // Update datasets
                            nodes.clear();
                            edges.clear();
                            nodes.add(data.nodes);
                            edges.add(data.edges);

                            if (!network) {
                                const options = {
                                    physics: { enabled: true, solver: 'forceAtlas2Based', forceAtlas2Based: { gravitationalConstant: -100, springLength: 10 } },
                                    interaction: { hover: true, navigationButtons: true }
                                };
                                network = new vis.Network(container, { nodes, edges }, options);
                                
                                // Auto-cluster
                                data.groups.forEach(g => {
                                    network.cluster({
                                        joinCondition: (n) => n.clusterGroup === g,
                                        clusterNodeProperties: { label: g, shape: 'box', color: '#fbbf24', font: { color: '#000' } }
                                    });
                                });
                            }
                        }
                    });
                </script>
            </body>
            </html>`;
    }

    public dispose() {
        TectPreviewPanel.currentPanel = undefined;
        this._panel.dispose();
        while (this._disposables.length) {
            const x = this._disposables.pop();
            if (x) x.dispose();
        }
    }
}