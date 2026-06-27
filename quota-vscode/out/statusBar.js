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
exports.QuotaStatusBar = void 0;
// @env node
const vscode = __importStar(require("vscode"));
const constants_1 = require("./constants");
const format_1 = require("./format");
class QuotaStatusBar {
    mainItem;
    trackItems = new Map();
    constructor() {
        this.mainItem = vscode.window.createStatusBarItem('quota.main', vscode.StatusBarAlignment.Right, 90);
        this.mainItem.name = constants_1.EXTENSION_NAME;
        this.mainItem.command = 'quota.openPanel';
    }
    dispose() {
        this.mainItem.dispose();
        for (const item of this.trackItems.values())
            item.dispose();
    }
    update(snapshot, config) {
        if (!config.statusBarEnabled) {
            this.mainItem.hide();
            this.hideTrackItems();
            return;
        }
        this.mainItem.text = '$(pulse) Quota';
        this.mainItem.tooltip = snapshot.warnings.length > 0
            ? `${snapshot.warnings[0]}\n\nClick to open Quota.`
            : `Click to open Quota.\nSource: ${snapshot.sourcePath}`;
        this.mainItem.show();
        const sortedTrackIds = [...config.statusBarItems].sort((a, b) => {
            const ai = constants_1.CANONICAL_TRACK_ORDER.indexOf(a);
            const bi = constants_1.CANONICAL_TRACK_ORDER.indexOf(b);
            return (ai === -1 ? 999 : ai) - (bi === -1 ? 999 : bi);
        });
        const selectedTrackIds = sortedTrackIds.slice(0, Math.max(0, config.statusBarMaxItems));
        const visibleTracks = selectedTrackIds
            .map((id) => snapshot.tracks.find((track) => track.id === id && config.enabledProviders.includes(track.providerId)))
            .filter((track) => track != null);
        const visibleIds = new Set(visibleTracks.map((track) => track.id));
        for (const [id, item] of this.trackItems.entries()) {
            if (!visibleIds.has(id))
                item.hide();
        }
        for (const track of visibleTracks) {
            const item = this.getTrackItem(track.id);
            item.text = `$(circle-filled) ${(0, format_1.statusBarLabel)(track, config.statusBarDisplay)}`;
            item.tooltip = [
                `${track.providerLabel}: ${track.label}`,
                track.accountLabel,
                track.error ? `Last error: ${track.error}` : undefined,
                snapshot.sourcePath,
            ].filter(Boolean).join('\n');
            item.backgroundColor = undefined;
            item.show();
        }
    }
    getTrackItem(id) {
        const existing = this.trackItems.get(id);
        if (existing)
            return existing;
        const canonicalIndex = constants_1.CANONICAL_TRACK_ORDER.indexOf(id);
        const priority = 89 - (canonicalIndex === -1 ? 50 : canonicalIndex);
        const item = vscode.window.createStatusBarItem(`quota.${id}`, vscode.StatusBarAlignment.Right, priority);
        item.name = `${constants_1.EXTENSION_NAME}: ${id}`;
        item.command = 'quota.openPanel';
        this.trackItems.set(id, item);
        return item;
    }
    hideTrackItems() {
        for (const item of this.trackItems.values())
            item.hide();
    }
}
exports.QuotaStatusBar = QuotaStatusBar;
//# sourceMappingURL=statusBar.js.map