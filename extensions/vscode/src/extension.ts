import * as path from 'path';
import * as os from 'os';
import * as fs from 'fs';
import * as vscode from 'vscode';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    TransportKind,
    RevealOutputChannelOn
} from 'vscode-languageclient/node';

let client: LanguageClient;

export function activate(context: vscode.ExtensionContext) {
    const outputChannel = vscode.window.createOutputChannel("Tect Language Server");
    outputChannel.appendLine("------------------------------------------------");
    outputChannel.appendLine(`[${new Date().toISOString()}] Extension activate() called.`);

    context.subscriptions.push(
        vscode.commands.registerCommand('tect.openPreview', () => {
            const editor = vscode.window.activeTextEditor;
            if (editor) {
                TectPreviewPanel.createOrShow(context.extensionUri, editor.document.uri);
            }
        })
    );

    context.subscriptions.push(
        vscode.window.onDidChangeActiveTextEditor(editor => {
            if (editor && editor.document.languageId === 'tect') {
                TectPreviewPanel.reviveOrUpdate(editor.document.uri, context.extensionUri);
            }
        })
    );

    const platform = os.platform();
    const arch = os.arch();

    outputChannel.appendLine(`Environment: Platform=${platform}, Arch=${arch}`);

    let binaryName: string;
    if (platform === 'win32') {
        binaryName = 'tect-x86_64-pc-windows-msvc.exe';
    } else if (platform === 'darwin') {
        binaryName = arch === 'arm64' ? 'tect-aarch64-apple-darwin' : 'tect-x86_64-apple-darwin';
    } else {
        binaryName = 'tect-x86_64-unknown-linux-gnu';
    }

    let serverModule = context.asAbsolutePath(path.join('bin', binaryName));
    let exists = fs.existsSync(serverModule);

    if (!exists) {
        outputChannel.appendLine("Path A failed. Attempting Path B (Dev/Fallback)...");
        const debugExec = platform === 'win32' ? 'tect.exe' : 'tect';
        serverModule = context.asAbsolutePath(path.join('..', '..', 'target', 'debug', debugExec));
        exists = fs.existsSync(serverModule);
    }

    if (!exists) {
        outputChannel.show(true);
        const msg = `CRITICAL: Tect Server binary NOT found. Searched for: ${binaryName}`;
        outputChannel.appendLine(msg);
        vscode.window.showErrorMessage(msg);
        return;
    }

    if (platform !== 'win32') {
        try {
            fs.chmodSync(serverModule, '755');
        } catch (e) {
            outputChannel.appendLine(`Permissions Warning: Failed to chmod binary: ${e}`);
        }
    }

    const serverOptions: ServerOptions = {
        run: { command: serverModule, args: ['serve'], transport: TransportKind.stdio },
        debug: { command: serverModule, args: ['serve'], transport: TransportKind.stdio }
    };

    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: 'file', language: 'tect' }],
        synchronize: { fileEvents: vscode.workspace.createFileSystemWatcher('**/*.tect') },
        outputChannel: outputChannel,
        revealOutputChannelOn: RevealOutputChannelOn.Error,
        initializationOptions: {}
    };

    try {
        client = new LanguageClient('tectServer', 'Tect Language Server', serverOptions, clientOptions);
        client.start().then(() => {
            outputChannel.appendLine(">>> LanguageClient Promise Resolved: Connection Established.");
            vscode.window.setStatusBarMessage("Tect Server: Active", 3000);

            client.onNotification("tect/analysisFinished", (params: { uri: string }) => {
                TectPreviewPanel.updateIfExists(params.uri);
            });
        }).catch(err => {
            outputChannel.show(true);
            vscode.window.showErrorMessage(`Tect Server failed to start: ${err}`);
        });

    } catch (e) {
        outputChannel.show(true);
        vscode.window.showErrorMessage(`Tect Extension Error: ${e}`);
    }
}

export function deactivate(): Thenable<void> | undefined {
    return client ? client.stop() : undefined;
}

class TectPreviewPanel {
    public static currentPanel: TectPreviewPanel | undefined;
    private readonly _panel: vscode.WebviewPanel;
    private _disposables: vscode.Disposable[] = [];
    private _uri: vscode.Uri;

    public static createOrShow(extensionUri: vscode.Uri, uri: vscode.Uri) {
        if (TectPreviewPanel.currentPanel) {
            TectPreviewPanel.currentPanel._panel.reveal(vscode.ViewColumn.Two);
            TectPreviewPanel.currentPanel.setUri(uri);
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

    public static reviveOrUpdate(uri: vscode.Uri, extensionUri: vscode.Uri) {
        if (TectPreviewPanel.currentPanel) {
            TectPreviewPanel.currentPanel.setUri(uri);
        }
    }

    public static async updateIfExists(uri: string) {
        if (TectPreviewPanel.currentPanel) {
            if (TectPreviewPanel.currentPanel._uri.toString() === uri) {
                TectPreviewPanel.currentPanel.update();
            }
        }
    }

    private constructor(panel: vscode.WebviewPanel, extensionUri: vscode.Uri, uri: vscode.Uri) {
        this._panel = panel;
        this._uri = uri;
        this._panel.onDidDispose(() => this.dispose(), null, this._disposables);

        this._panel.webview.onDidReceiveMessage(
            message => {
                switch (message.command) {
                    case 'webviewReady':
                        this.update();
                        return;
                }
            },
            null,
            this._disposables
        );

        this._panel.webview.html = this._getHtmlForWebview();
        this._updateTitle();
    }

    public setUri(uri: vscode.Uri) {
        if (this._uri.toString() !== uri.toString()) {
            this._uri = uri;
            this._updateTitle();
            this.update();
        }
    }

    private _updateTitle() {
        const fileName = path.basename(this._uri.fsPath);
        this._panel.title = `Tect: ${fileName}`;
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
                    const vscode = acquireVsCodeApi();
                    const container = document.getElementById('mynetwork');
                    let network = null;
                    let nodes = new vis.DataSet([]);
                    let edges = new vis.DataSet([]);
                    let currentGroups = [];
                    // Using map passed from server now
                    let currentGroupColors = {};

                    const clusterBy = (g) => ({
                        joinCondition: (n) => n.clusterGroup === g,
                        clusterNodeProperties: { 
                            id: 'c:' + g,
                            label: g, 
                            shape: 'box',
                            margin: 10,
                            color: { 
                                // Use authoritative color from server, or fallback
                                background: currentGroupColors[g] || '#fbbf24', 
                                border: '#fff' 
                            }, 
                            font: { color: '#fff', size: 16, face: 'sans-serif', strokeWidth: 0 } 
                        }
                    });

                    window.addEventListener('message', event => {
                        const message = event.data;
                        if (message.command === 'update' && message.data) {
                            const data = message.data;
                            currentGroups = data.groups || [];
                            currentGroupColors = data.groupColors || {};

                            // Initialize Network if first run
                            if (!network) {
                                const options = {
                                    physics: { 
                                        enabled: true, 
                                        solver: 'forceAtlas2Based', 
                                        forceAtlas2Based: { gravitationalConstant: -100, springLength: 10 } 
                                    },
                                    interaction: { hover: true, navigationButtons: true }
                                };
                                network = new vis.Network(container, { nodes, edges }, options);
                                
                                network.on("doubleClick", (params) => {
                                    if (params.nodes.length > 0) {
                                        const nodeId = params.nodes[0];
                                        if (network.isCluster(nodeId)) {
                                            network.openCluster(nodeId);
                                        } else {
                                            const node = nodes.get(nodeId);
                                            if (node && node.clusterGroup) {
                                                network.cluster(clusterBy(node.clusterGroup));
                                            }
                                        }
                                    }
                                });
                            }

                            // Differential Update to preserve Physics
                            // The server sends stable UIDs based on artifact names.
                            nodes.update(data.nodes);
                            edges.update(data.edges);
                            
                            // Cleanup: Remove nodes/edges that are no longer in the new data
                            const newIds = new Set(data.nodes.map(n => n.id));
                            const newEdgeIds = new Set(data.edges.map(e => e.id));
                            
                            const removeNodes = nodes.getIds().filter(id => !newIds.has(id));
                            const removeEdges = edges.getIds().filter(id => !newEdgeIds.has(id));
                            
                            nodes.remove(removeNodes);
                            edges.remove(removeEdges);
                        }
                    });

                    vscode.postMessage({ command: 'webviewReady' });
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