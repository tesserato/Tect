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
    // 0. Create the Output Channel
    const outputChannel = vscode.window.createOutputChannel("Tect Language Server");
    outputChannel.appendLine("------------------------------------------------");
    outputChannel.appendLine(`[${new Date().toISOString()}] Extension activate() called.`);
    // 1. Show a visible popup to confirm activation event fired
    vscode.window.showInformationMessage("Tect Extension is activating!");


    // 2. Register Commands
    context.subscriptions.push(
        vscode.commands.registerCommand('tect.openPreview', () => {
            const editor = vscode.window.activeTextEditor;
            if (editor) {
                TectPreviewPanel.createOrShow(context.extensionUri, editor.document.uri);
            }
        })
    );

    // 3. Listen for active editor changes to update the preview context
    context.subscriptions.push(
        vscode.window.onDidChangeActiveTextEditor(editor => {
            if (editor && editor.document.languageId === 'tect') {
                TectPreviewPanel.reviveOrUpdate(editor.document.uri, context.extensionUri);
            }
        })
    );

    // --- Production Binary Discovery ---
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

    // --- Resolve Binary Path ---
    // 1. Try production path (inside extension folder)
    let serverModule = context.asAbsolutePath(path.join('bin', binaryName));
    let exists = fs.existsSync(serverModule);

    // 2. Try development fallback (relative to source for local dev)
    if (!exists) {
        outputChannel.appendLine("Path A failed. Attempting Path B (Dev/Fallback)...");
        const debugExec = platform === 'win32' ? 'tect.exe' : 'tect';
        serverModule = context.asAbsolutePath(path.join('..', '..', 'target', 'debug', debugExec));
        exists = fs.existsSync(serverModule);
    }

    // 3. Final Critical Check
    if (!exists) {
        outputChannel.show(true);
        const msg = `CRITICAL: Tect Server binary NOT found. Searched for: ${binaryName}`;
        outputChannel.appendLine(msg);
        vscode.window.showErrorMessage(msg);
        return;
    }

    // --- Permissions Check (Linux/Mac) ---
    if (platform !== 'win32') {
        try {
            fs.chmodSync(serverModule, '755');
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
        revealOutputChannelOn: RevealOutputChannelOn.Error,
        initializationOptions: {}
    };

    // --- Start Client ---
    try {
        client = new LanguageClient('tectServer', 'Tect Language Server', serverOptions, clientOptions);
        client.start().then(() => {
            outputChannel.appendLine(">>> LanguageClient Promise Resolved: Connection Established.");
            vscode.window.setStatusBarMessage("Tect Server: Active", 3000);

            // Listen for custom notifications from server to update graph
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

/**
 * Manages the Tect Architecture Preview Webview.
 */
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

    /**
     * Updates the existing panel to point to a new file if one exists,
     * otherwise does nothing.
     */
    public static reviveOrUpdate(uri: vscode.Uri, extensionUri: vscode.Uri) {
        if (TectPreviewPanel.currentPanel) {
            TectPreviewPanel.currentPanel.setUri(uri);
        }
    }

    /**
     * Updates the content of the webview if it is currently viewing the specified URI.
     */
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

        // Handle messages from the webview
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
        // Only update if the URI actually changed
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
                            
                            // 1. Destroy existing network to ensure clean state for new file/clusters
                            if (network) {
                                network.destroy();
                                network = null;
                            }

                            // 2. Reset and populate datasets
                            nodes.clear();
                            edges.clear();
                            nodes.add(data.nodes);
                            edges.add(data.edges);

                            // 3. Create fresh Network instance
                            const options = {
                                physics: { 
                                    enabled: true, 
                                    solver: 'forceAtlas2Based', 
                                    forceAtlas2Based: { gravitationalConstant: -100, springLength: 10 } 
                                },
                                interaction: { hover: true, navigationButtons: true }
                            };
                            
                            network = new vis.Network(container, { nodes, edges }, options);
                            
                            // 4. Apply Group Clustering
                            data.groups.forEach(g => {
                                network.cluster(clusterBy(g));
                            });

                            // 5. Interaction Events
                            network.on("doubleClick", (params) => {
                                if (params.nodes.length > 0) {
                                    const nodeId = params.nodes[0];
                                    if (network.isCluster(nodeId)) {
                                        network.openCluster(nodeId);
                                    } else {
                                        // Optional: Re-collapse if clicking inside
                                        const node = nodes.get(nodeId);
                                        if (node && node.clusterGroup) {
                                            network.cluster(clusterBy(node.clusterGroup));
                                        }
                                    }
                                }
                            });
                        }
                    });

                    // Signal to VS Code that the webview script is loaded
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