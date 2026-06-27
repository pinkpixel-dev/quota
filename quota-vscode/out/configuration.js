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
exports.readConfiguration = readConfiguration;
// @env node
const vscode = __importStar(require("vscode"));
const constants_1 = require("./constants");
const TRACK_IDS = [
    'githubCopilot.premium',
    'githubCopilot.chat',
    'githubCopilot.inline',
    'codex.primary',
    'codex.weekly',
    'claude.fiveHour',
    'claude.weekly',
    'claude.weeklySonnet',
    'claude.extraUsage',
    'antigravity.gemini',
    'antigravity.geminiWeekly',
    'antigravity.claude',
    'antigravity.claudeWeekly',
    'kiro.promptCredits',
];
function isProviderId(value) {
    return constants_1.PROVIDER_ORDER.includes(value);
}
function isTrackId(value) {
    return TRACK_IDS.includes(value);
}
function readStringArray(section, key) {
    const value = section.get(key);
    return Array.isArray(value) ? value.filter((item) => typeof item === 'string') : [];
}
function readConfiguration() {
    const section = vscode.workspace.getConfiguration('quota');
    const dataSource = section.get('dataSource', 'extensionAccounts');
    const summaryPath = section.get('summaryPath', '').trim() || constants_1.DEFAULT_SUMMARY_PATH;
    const enabledProviders = readStringArray(section, 'providers.enabled').filter(isProviderId);
    const statusBarItems = readStringArray(section, 'statusBar.items').filter(isTrackId);
    const statusBarDisplay = section.get('statusBar.display', 'percentUsed');
    return {
        dataSource,
        summaryPath,
        enabledProviders: enabledProviders.length > 0 ? enabledProviders : constants_1.PROVIDER_ORDER,
        statusBarEnabled: section.get('statusBar.enabled', true),
        statusBarItems,
        statusBarDisplay,
        statusBarMaxItems: section.get('statusBar.maxItems', 3),
        refreshIntervalSeconds: section.get('refresh.intervalSeconds', 120),
    };
}
//# sourceMappingURL=configuration.js.map