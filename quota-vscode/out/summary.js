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
exports.expandHome = expandHome;
exports.loadQuotaSnapshot = loadQuotaSnapshot;
// @env node
const fs = __importStar(require("node:fs/promises"));
const os = __importStar(require("node:os"));
const path = __importStar(require("node:path"));
const antigravityUsage_1 = require("./antigravityUsage");
const constants_1 = require("./constants");
function isRecord(value) {
    return typeof value === 'object' && value !== null && !Array.isArray(value);
}
function asArray(value) {
    return Array.isArray(value) ? value.filter(isRecord) : [];
}
function asNumber(value) {
    return typeof value === 'number' && Number.isFinite(value) ? value : undefined;
}
function asString(value) {
    return typeof value === 'string' && value.trim().length > 0 ? value : undefined;
}
function clampPercent(value) {
    if (value == null)
        return undefined;
    return Math.min(100, Math.max(0, Math.round(value)));
}
function usedFromRemaining(value) {
    const remaining = clampPercent(asNumber(value));
    return remaining == null ? undefined : 100 - remaining;
}
function normalizeTimestamp(value) {
    const timestamp = asNumber(value);
    if (timestamp == null)
        return undefined;
    return timestamp > 0 && timestamp < 10_000_000_000 ? timestamp * 1000 : timestamp;
}
function accountLabel(account) {
    return (asString(account.email)
        ?? asString(account.githubLogin)
        ?? asString(account.githubEmail)
        ?? asString(account.displayName)
        ?? asString(account.name)
        ?? 'Connected account');
}
function makeTrack(id, providerId, account, label, percentUsed, percentRemaining, resetAt) {
    return {
        id,
        providerId,
        providerLabel: constants_1.PROVIDER_LABELS[providerId],
        label,
        accountLabel: accountLabel(account),
        percentUsed: clampPercent(percentUsed),
        percentRemaining: clampPercent(percentRemaining),
        resetAt,
        updatedAt: normalizeTimestamp(account.usageUpdatedAt),
        error: asString(account.quotaQueryLastError) ?? null,
    };
}
function tracksFromCopilot(accounts) {
    return accounts.flatMap((account) => {
        const usage = isRecord(account.usage) ? account.usage : {};
        const resetAt = normalizeTimestamp(usage.allowanceResetAt);
        return [
            makeTrack('githubCopilot.premium', 'githubCopilot', account, 'Premium requests', asNumber(usage.premiumRequestsUsedPercent), undefined, resetAt),
            makeTrack('githubCopilot.chat', 'githubCopilot', account, 'Chat messages', asNumber(usage.chatMessagesUsedPercent), undefined, resetAt),
            makeTrack('githubCopilot.inline', 'githubCopilot', account, 'Inline suggestions', asNumber(usage.inlineSuggestionsUsedPercent), undefined, resetAt),
        ];
    });
}
function tracksFromCodex(accounts) {
    return accounts.flatMap((account) => {
        const quota = isRecord(account.quota) ? account.quota : {};
        return [
            makeTrack('codex.primary', 'codex', account, '5h usage', usedFromRemaining(quota.hourlyRemainingPercent), asNumber(quota.hourlyRemainingPercent), normalizeTimestamp(quota.hourlyResetAt)),
            makeTrack('codex.weekly', 'codex', account, 'Weekly usage', usedFromRemaining(quota.weeklyRemainingPercent), asNumber(quota.weeklyRemainingPercent), normalizeTimestamp(quota.weeklyResetAt)),
        ];
    });
}
function tracksFromClaude(accounts) {
    return accounts.flatMap((account) => {
        const quota = isRecord(account.quota) ? account.quota : {};
        return [
            makeTrack('claude.fiveHour', 'claude', account, '5h usage', usedFromRemaining(quota.fiveHourRemainingPercent), asNumber(quota.fiveHourRemainingPercent), normalizeTimestamp(quota.fiveHourResetAt)),
            makeTrack('claude.weekly', 'claude', account, 'Weekly usage', usedFromRemaining(quota.weeklyRemainingPercent), asNumber(quota.weeklyRemainingPercent), normalizeTimestamp(quota.weeklyResetAt)),
        ];
    });
}
function tracksFromAntigravity(accounts) {
    return accounts.flatMap((account) => {
        const quota = isRecord(account.quota) ? account.quota : {};
        const geminiFiveHour = isRecord(quota.geminiFiveHour) ? quota.geminiFiveHour : {};
        const thirdPartyFiveHour = isRecord(quota.thirdPartyFiveHour) ? quota.thirdPartyFiveHour : {};
        const credits = asArray(account.credits)
            .map((item) => {
            const creditType = asString(item.creditType);
            const creditAmount = asString(item.creditAmount);
            if (!creditType || !creditAmount)
                return undefined;
            return {
                creditType,
                creditAmount,
                minimumCreditAmountForUsage: asString(item.minimumCreditAmountForUsage),
            };
        })
            .filter((item) => item != null);
        const creditsValue = (0, antigravityUsage_1.formatAntigravityCredits)(credits);
        return [
            makeTrack('antigravity.gemini', 'antigravity', account, 'Gemini Models', usedFromRemaining(geminiFiveHour.remainingPercent), asNumber(geminiFiveHour.remainingPercent), normalizeTimestamp(geminiFiveHour.resetAt)),
            makeTrack('antigravity.claude', 'antigravity', account, 'Claude/GPT models', usedFromRemaining(thirdPartyFiveHour.remainingPercent), asNumber(thirdPartyFiveHour.remainingPercent), normalizeTimestamp(thirdPartyFiveHour.resetAt)),
            creditsValue
                ? {
                    id: 'antigravity.credits',
                    providerId: 'antigravity',
                    providerLabel: constants_1.PROVIDER_LABELS.antigravity,
                    label: 'Available AI Credits',
                    accountLabel: accountLabel(account),
                    valueLabel: creditsValue,
                    updatedAt: normalizeTimestamp(account.usageUpdatedAt),
                    error: asString(account.quotaQueryLastError) ?? null,
                }
                : undefined,
        ].filter((track) => track != null);
    });
}
function tracksFromKiro(accounts) {
    return accounts.map((account) => {
        const used = asNumber(account.creditsUsed);
        const total = asNumber(account.creditsTotal);
        const percentUsed = used != null && total != null && total > 0 ? (used / total) * 100 : undefined;
        return makeTrack('kiro.promptCredits', 'kiro', account, 'Prompt credits', percentUsed, percentUsed == null ? undefined : 100 - percentUsed, normalizeTimestamp(account.usageResetAt));
    });
}
function payloadToTracks(payload) {
    const providers = isRecord(payload.providers) ? payload.providers : {};
    return [
        ...tracksFromCopilot(asArray(providers.githubCopilot)),
        ...tracksFromCodex(asArray(providers.codex)),
        ...tracksFromClaude(asArray(providers.claude)),
        ...tracksFromAntigravity(asArray(providers.antigravity)),
        ...tracksFromKiro(asArray(providers.kiro)),
    ].filter((track) => track.percentUsed != null || track.percentRemaining != null || track.valueLabel || track.error);
}
function expandHome(inputPath) {
    if (inputPath === '~')
        return os.homedir();
    if (inputPath.startsWith(`~${path.sep}`))
        return path.join(os.homedir(), inputPath.slice(2));
    if (inputPath.startsWith('~/'))
        return path.join(os.homedir(), inputPath.slice(2));
    return inputPath;
}
async function loadQuotaSnapshot(inputPath) {
    const sourcePath = expandHome(inputPath);
    const warnings = [];
    try {
        const raw = await fs.readFile(sourcePath, 'utf8');
        const parsed = JSON.parse(raw);
        if (!isRecord(parsed)) {
            return { sourcePath, tracks: [], warnings: ['The summary file is not a JSON object.'] };
        }
        return {
            sourcePath,
            exportedAt: asString(parsed.exportedAt),
            tracks: payloadToTracks(parsed),
            warnings,
        };
    }
    catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        return {
            sourcePath,
            tracks: [],
            warnings: [`Could not read the safe summary file: ${message}`],
        };
    }
}
//# sourceMappingURL=summary.js.map