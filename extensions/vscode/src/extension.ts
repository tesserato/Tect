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
    // 1. Create and FORCE SHOW the Output Channel
    const outputChannel = vscode.window.createOutputChannel("Tect Language Server");
    outputChannel.show(true); // Bring to front
    outputChannel.appendLine("------------------------------------------------");
    outputChannel.appendLine(`[${new Date().toISOString()}] Extension activate() called.`);

    // 2. Show a visible popup to confirm activation event fired
    vscode.window.showInformationMessage("Tect Extension is activating!");

    context.subscriptions.push(
        vscode.commands.registerCommand('tect.openPreview', () => {
            const editor = vscode.window.activeTextEditor;
            if (editor) {
                TectPreviewPanel.createOrShow(context.extensionUri, editor.document.uri);
            }
        })
    );

    // --- Production Binary Discovery ---
    const platform = os.platform();
    const arch = os.arch();

    outputChannel.appendLine(`Environment: Platform=${platform}, Arch=${arch}`);
    outputChannel.appendLine(`Extension Path: ${context.extensionPath}`);

    let binaryName: string;
    if (platform === 'win32') {
        binaryName = 'tect-x86_64-pc-windows-msvc.exe';
    } else if (platform === 'darwin') {
        binaryName = arch === 'arm64' ? 'tect-aarch64-apple-darwin' : 'tect-x86_64-apple-darwin';
    } else {
        binaryName = 'tect-x86_64-unknown-linux-gnu';
    }

    // --- Resolve Binary Path ---
    // 1. Try production path (inside extension folder)
    let serverModule = context.asAbsolutePath(path.join('bin', binaryName));
    let exists = fs.existsSync(serverModule);
    outputChannel.appendLine(`Checking Path A (Production): "${serverModule}" -> Exists: ${exists}`);

    // 2. Try development fallback (relative to source)
    if (!exists) {
        outputChannel.appendLine("Path A failed. Attempting Path B (Dev/Fallback)...");
        const debugExec = platform === 'win32' ? 'tect.exe' : 'tect';
        // Adjust this path if your folder structure is different
        // Current assumption: extension.js is in /editors/vscode/out/
        // Target is /target/debug/
        serverModule = context.asAbsolutePath(path.join('..', '..', 'target', 'debug', debugExec));
        exists = fs.existsSync(serverModule);
        outputChannel.appendLine(`Checking Path B (Dev): "${serverModule}" -> Exists: ${exists}`);
    }

    // 3. Final Critical Check
    if (!exists) {
        const msg = `CRITICAL: Tect Server binary NOT found. Searched for: ${binaryName}`;
        outputChannel.appendLine(msg);
        vscode.window.showErrorMessage(msg);
        return;
    }

    outputChannel.appendLine(`Resolved Server Binary: ${serverModule}`);

    // --- Permissions Check (Linux/Mac) ---
    if (platform !== 'win32') {
        try {
            fs.chmodSync(serverModule, '755');
            outputChannel.appendLine("Permissions: chmod 755 applied successfully.");
        } catch (e) {
            outputChannel.appendLine(`Permissions Warning: Failed to chmod binary: ${e}`);
        }
    }

    // --- Server Options ---
    const serverOptions: ServerOptions = {
        run: { command: serverModule, args: ['serve'], transport: TransportKind.stdio },
        debug: { command: serverModule, args: ['serve'], transport: TransportKind.stdio }
    };

    // --- Client Options ---
    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: 'file', language: 'tect' }],
        synchronize: { fileEvents: vscode.workspace.createFileSystemWatcher('**/*.tect') },
        outputChannel: outputChannel,
        revealOutputChannelOn: RevealOutputChannelOn.Info, // Show on info/error
        initializationOptions: {},
        errorHandler: {
            error: (error, message, count) => {
                outputChannel.appendLine(`LSP Error: ${error} | ${message}`);
                return { action: 1 }; // Shutdown
            },
            closed: () => {
                outputChannel.appendLine("LSP Connection Closed unexpectedly.");
                return { action: 1 }; // Do not restart
            }
        }
    };

    // --- Start Client ---
    try {
        client = new LanguageClient('tectServer', 'Tect Language Server', serverOptions, clientOptions);

        outputChannel.appendLine("Starting LanguageClient...");
        client.start().then(() => {
            outputChannel.appendLine(">>> LanguageClient Promise Resolved: Connection Established.");
            vscode.window.setStatusBarMessage("Tect Server: Active", 3000);

            client.onNotification("tect/analysisFinished", (params: { uri: string }) => {
                outputChannel.appendLine(`Analysis finished for: ${params.uri}`);
                TectPreviewPanel.updateIfExists(params.uri);
            });
        }).catch(err => {
            outputChannel.appendLine(`!!! LanguageClient Start Failed: ${err}`);
            vscode.window.showErrorMessage(`Tect Server failed to start: ${err}`);
        });

    } catch (e) {
        outputChannel.appendLine(`Exception during client creation: ${e}`);
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

                    // Define clustering helper
                    const clusterBy = (groupName) => ({
                        joinCondition: (n) => n.clusterGroup === groupName,
                        clusterNodeProperties: { 
                            label: groupName, 
                            shape: 'box', 
                            color: '#fbbf24', 
                            font: { color: '#000' } 
                        }
                    });

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
                                    network.cluster(clusterBy(g));
                                });

                                // Event: Double click to expand/collapse groups
                                network.on("doubleClick", (params) => {
                                    if (params.nodes.length > 0) {
                                        const nodeId = params.nodes[0];
                                        if (network.isCluster(nodeId)) {
                                            network.openCluster(nodeId);
                                        } else {
                                            // Optional: Double clicking a node inside a group re-collapses it
                                            const node = nodes.get(nodeId);
                                            if (node && node.clusterGroup) {
                                                network.cluster(clusterBy(node.clusterGroup));
                                            }
                                        }
                                    }
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