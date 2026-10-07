"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.chooseStatusBarItems = chooseStatusBarItems;
// @env node
const vscode = __importStar(require("vscode"));
const constants_1 = require("./constants");
const statusBarSelection_1 = require("./statusBarSelection");
const MAX_STATUS_BAR_ITEMS = 8;
function buildPickItems(snapshot, config) {
    const items = [];
    for (const providerId of constants_1.PROVIDER_ORDER) {
        if (!config.enabledProviders.includes(providerId))
            continue;
        const providerTracks = snapshot.tracks
            .filter((track) => track.providerId === providerId)
            .sort((a, b) => constants_1.CANONICAL_TRACK_ORDER.indexOf(a.id) - constants_1.CANONICAL_TRACK_ORDER.indexOf(b.id));
        const accountIds = [...new Set(providerTracks.map((track) => track.accountId))];
        for (const accountId of accountIds) {
            const accountTracks = providerTracks.filter((track) => track.accountId === accountId);
            items.push({
                label: `${constants_1.PROVIDER_LABELS[providerId]} · ${accountTracks[0].accountLabel}`,
                kind: vscode.QuickPickItemKind.Separator,
            });
            for (const track of accountTracks) {
                items.push({
                    label: constants_1.TRACK_LABELS[track.id] ?? track.label,
                    description: track.accountLabel,
                    picked: (0, statusBarSelection_1.isTrackSelected)(config.statusBarItems, track, snapshot.tracks),
                    track,
                });
            }
        }
    }
    return items;
}
/** Shows every connected account's tracks and saves the chosen ones as `trackId@accountId` entries. */
async function chooseStatusBarItems(snapshot, config) {
    const items = buildPickItems(snapshot, config);
    if (!items.some((item) => item.track)) {
        void vscode.window.showInformationMessage('Connect an account first, then choose which quotas show in the status bar.');
        return false;
    }
    const picked = await vscode.window.showQuickPick(items, {
        title: 'Quota: Status Bar Items',
        placeHolder: 'Pick the quotas to show in the status bar',
        canPickMany: true,
        matchOnDescription: true,
    });
    if (!picked)
        return false;
    const offeredProviders = new Set(items.filter((item) => item.track).map((item) => item.track.providerId));
    // Keep entries for providers this list did not show (disabled or no data yet). Entries for
    // shown providers are replaced by the selection, which also drops disconnected accounts.
    const kept = config.statusBarItems.filter((value) => {
        const entry = (0, statusBarSelection_1.parseStatusBarEntry)(value);
        return entry != null && !offeredProviders.has(entry.trackId.slice(0, entry.trackId.indexOf('.')));
    });
    const next = [...kept, ...picked.filter((item) => item.track).map((item) => (0, statusBarSelection_1.statusBarEntryKey)(item.track))];
    const section = vscode.workspace.getConfiguration('quota');
    const target = section.inspect('statusBar.items')?.workspaceValue !== undefined
        ? vscode.ConfigurationTarget.Workspace
        : vscode.ConfigurationTarget.Global;
    await section.update('statusBar.items', next, target);
    if (next.length > config.statusBarMaxItems) {
        const raise = Math.min(MAX_STATUS_BAR_ITEMS, next.length);
        const action = raise > config.statusBarMaxItems ? `Show ${raise}` : undefined;
        const message = `Only the first ${config.statusBarMaxItems} of ${next.length} selected quotas fit the status bar limit.`;
        const selected = action
            ? await vscode.window.showWarningMessage(message, action)
            : await vscode.window.showWarningMessage(message);
        if (action && selected === action)
            await section.update('statusBar.maxItems', raise, vscode.ConfigurationTarget.Global);
    }
    return true;
}
//# sourceMappingURL=statusBarPicker.js.map