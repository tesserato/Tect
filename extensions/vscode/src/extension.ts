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
                    case 'saveImage':
                        this.saveImage(message.data);
                        return;
                    case 'exportContent':
                        this.exportContent(message.format);
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
            
            const config = vscode.workspace.getConfiguration("tect");
            const visConfig = config.get("visConfig") || {};

            this._panel.webview.postMessage({ 
                command: 'update', 
                data: visData,
                config: visConfig 
            });
        } catch (e) {
            console.error("Failed to fetch graph data", e);
        }
    }

    private async saveImage(base64Data: string) {
        const matches = base64Data.match(/^data:([A-Za-z-+\/]+);base64,(.+)$/);
        if (!matches || matches.length !== 3) {
            return;
        }

        const buffer = Buffer.from(matches[2], 'base64');
        const uri = await vscode.window.showSaveDialog({
            defaultUri: vscode.Uri.file('architecture.png'),
            filters: { 'Images': ['png'] }
        });

        if (uri) {
            fs.writeFile(uri.fsPath, buffer, (err) => {
                if (err) {
                    vscode.window.showErrorMessage(`Failed to save image: ${err.message}`);
                } else {
                    vscode.window.showInformationMessage('Graph saved successfully!');
                }
            });
        }
    }

    private async exportContent(format: string) {
        if (!client) return;
        
        let fileExt = "";
        let filterName = "";
        
        switch (format) {
            case 'dot': fileExt = 'dot'; filterName = 'Graphviz DOT'; break;
            case 'mermaid': fileExt = 'mmd'; filterName = 'Mermaid Diagram'; break;
            case 'tex': fileExt = 'tex'; filterName = 'LaTeX TikZ'; break;
            case 'json': fileExt = 'json'; filterName = 'JSON Data'; break;
            default: return;
        }

        try {
            const content = await client.sendRequest<string>("tect/exportGraph", { 
                uri: this._uri.toString(),
                format: format 
            });

            const uri = await vscode.window.showSaveDialog({
                defaultUri: vscode.Uri.file(`architecture.${fileExt}`),
                filters: { [filterName]: [fileExt] }
            });

            if (uri) {
                fs.writeFile(uri.fsPath, content, (err) => {
                    if (err) {
                        vscode.window.showErrorMessage(`Failed to save ${format.toUpperCase()}: ${err.message}`);
                    } else {
                        vscode.window.showInformationMessage(`${format.toUpperCase()} saved successfully!`);
                    }
                });
            }
        } catch (e) {
            vscode.window.showErrorMessage(`Failed to generate export: ${e}`);
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
                    body { background-color: #0b0e14; color: #e0e0e0; margin: 0; padding: 0; overflow: hidden; height: 100vh; font-family: sans-serif; user-select: none; }
                    #mynetwork { width: 100%; height: 100vh; }
                    
                    /* Toolbar */
                    #toolbar {
                        position: absolute;
                        top: 20px;
                        right: 20px;
                        display: flex;
                        gap: 10px;
                        z-index: 100;
                    }
                    .btn {
                        background: #1f2937;
                        border: 1px solid #374151;
                        color: #e5e7eb;
                        padding: 6px 12px;
                        border-radius: 4px;
                        cursor: pointer;
                        font-size: 13px;
                        transition: background 0.2s;
                    }
                    .btn:hover { background: #374151; }
                    .btn.active { background: #3b82f6; border-color: #2563eb; color: white; }

                    /* Custom Context Menu */
                    #context-menu {
                        position: absolute;
                        display: none;
                        background: #1f2937;
                        border: 1px solid #374151;
                        box-shadow: 0 4px 6px rgba(0,0,0,0.3);
                        border-radius: 4px;
                        padding: 5px 0;
                        z-index: 1000;
                        min-width: 150px;
                    }
                    .context-item {
                        padding: 8px 16px;
                        cursor: pointer;
                        color: #e5e7eb;
                        font-size: 13px;
                    }
                    .context-item:hover {
                        background: #374151;
                    }
                    .context-divider {
                        height: 1px;
                        background: #374151;
                        margin: 4px 0;
                    }
                </style>
            </head>
            <body>
                <div id="toolbar">
                    <button class="btn" id="btn-physics" onclick="togglePhysics()">Pause Physics</button>
                    <button class="btn" onclick="network.fit({animation:true})">Fit Graph</button>
                    <button class="btn" onclick="expandAll()">Expand All</button>
                    <button class="btn" onclick="collapseAll()">Collapse All</button>
                </div>

                <div id="mynetwork"></div>
                <div id="context-menu">
                    <div class="context-item" onclick="exportImage()">Export as PNG (Canvas)</div>
                    <div class="context-divider"></div>
                    <div class="context-item" onclick="requestExport('mermaid')">Export as Mermaid</div>
                    <div class="context-item" onclick="requestExport('dot')">Export as DOT</div>
                    <div class="context-item" onclick="requestExport('tex')">Export as LaTeX</div>
                    <div class="context-item" onclick="requestExport('json')">Export as JSON</div>
                </div>

                <script>
                    const vscode = acquireVsCodeApi();
                    const container = document.getElementById('mynetwork');
                    const ctxMenu = document.getElementById('context-menu');
                    let network = null;
                    let nodes = new vis.DataSet([]);
                    let edges = new vis.DataSet([]);
                    let currentGroups = [];
                    let currentGroupColors = {};
                    let physicsEnabled = true;

                    const clusterBy = (g) => ({
                        joinCondition: (n) => n.clusterGroup === g,
                        clusterNodeProperties: { 
                            id: 'c:' + g,
                            label: g, 
                            shape: 'box',
                            margin: 10,
                            color: { 
                                background: currentGroupColors[g] || '#fbbf24', 
                                border: '#fff' 
                            }, 
                            font: { color: '#fff', size: 16, face: 'sans-serif', strokeWidth: 0 } 
                        }
                    });

                    function togglePhysics() {
                        physicsEnabled = !physicsEnabled;
                        network.setOptions({ physics: { enabled: physicsEnabled } });
                        const btn = document.getElementById('btn-physics');
                        btn.textContent = physicsEnabled ? "Pause Physics" : "Resume Physics";
                        btn.classList.toggle('active', !physicsEnabled);
                    }

                    function expandAll() {
                        if(!network) return;
                        // Iterate explicitly to handle potential nested or stubborn clusters
                        let clusterIds = network.body.nodeIndices.filter(id => network.isCluster(id));
                        // Limit iterations to prevent freezing if cycles occurred (rare in Vis.js)
                        let safety = 0;
                        while(clusterIds.length > 0 && safety < 10) {
                            clusterIds.forEach(id => {
                                try { network.openCluster(id); } catch(e){}
                            });
                            clusterIds = network.body.nodeIndices.filter(id => network.isCluster(id));
                            safety++;
                        }
                    }

                    function collapseAll() {
                        if(!network) return;
                        // Fully expand first to ensure a clean slate
                        expandAll();
                        // Re-cluster based on authoritative group list
                        currentGroups.forEach(g => {
                            try {
                                network.cluster(clusterBy(g));
                            } catch(e) { console.error(e); }
                        });
                    }

                    function exportImage() {
                        ctxMenu.style.display = 'none';
                        const canvas = container.getElementsByTagName('canvas')[0];
                        if (canvas) {
                            const data = canvas.toDataURL('image/png');
                            vscode.postMessage({ command: 'saveImage', data: data });
                        }
                    }

                    function requestExport(fmt) {
                        ctxMenu.style.display = 'none';
                        vscode.postMessage({ command: 'exportContent', format: fmt });
                    }

                    // Context Menu Logic
                    container.addEventListener('contextmenu', (e) => {
                        e.preventDefault();
                        ctxMenu.style.top = e.offsetY + 'px';
                        ctxMenu.style.left = e.offsetX + 'px';
                        ctxMenu.style.display = 'block';
                    });
                    
                    document.addEventListener('click', () => {
                        ctxMenu.style.display = 'none';
                    });

                    window.addEventListener('message', event => {
                        const message = event.data;
                        if (message.command === 'update' && message.data) {
                            const data = message.data;
                            const userConfig = message.config || {};
                            
                            currentGroups = data.groups || [];
                            currentGroupColors = data.groupColors || {};

                            if (!network) {
                                const defaultOptions = {
                                    physics: { 
                                        enabled: true, 
                                        solver: 'forceAtlas2Based', 
                                        forceAtlas2Based: { gravitationalConstant: -100, springLength: 10 } 
                                    },
                                    interaction: { hover: true, navigationButtons: true },
                                    layout: { improvedLayout: true }
                                };

                                // Merge user config
                                const options = { ...defaultOptions, ...userConfig };
                                
                                // Ensure physics matches user config if provided
                                if (userConfig.physics && userConfig.physics.enabled !== undefined) {
                                    physicsEnabled = userConfig.physics.enabled;
                                    document.getElementById('btn-physics').textContent = physicsEnabled ? "Pause Physics" : "Resume Physics";
                                }

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
                            } else if (Object.keys(userConfig).length > 0) {
                                network.setOptions(userConfig);
                            }

                            nodes.update(data.nodes);
                            edges.update(data.edges);
                            
                            const newIds = new Set(data.nodes.map(n => n.id));
                            const newEdgeIds = new Set(data.edges.map(e => e.id));
                            
                            nodes.remove(nodes.getIds().filter(id => !newIds.has(id)));
                            edges.remove(edges.getIds().filter(id => !newEdgeIds.has(id)));
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