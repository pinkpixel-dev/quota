"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.parseGrokTimestamp = parseGrokTimestamp;
exports.humanizeGrokTier = humanizeGrokTier;
exports.humanizeGrokPeriod = humanizeGrokPeriod;
exports.buildGrokUsageSummary = buildGrokUsageSummary;
exports.formatGrokAmount = formatGrokAmount;
function isRecord(value) {
    return typeof value === 'object' && value !== null && !Array.isArray(value);
}
function getPath(root, path) {
    let current = root;
    for (const key of path) {
        if (!isRecord(current))
            return undefined;
        current = current[key];
    }
    return current;
}
function pickString(root, paths) {
    for (const path of paths) {
        const value = getPath(root, path);
        if (typeof value === 'string' && value.trim())
            return value.trim();
    }
    return undefined;
}
function pickNumber(root, paths) {
    for (const path of paths) {
        const value = getPath(root, path);
        if (typeof value === 'number' && Number.isFinite(value))
            return value;
        if (typeof value === 'string') {
            const parsed = Number.parseFloat(value.trim());
            if (Number.isFinite(parsed))
                return parsed;
        }
    }
    return undefined;
}
function parseGrokTimestamp(value) {
    if (typeof value === 'number' && Number.isFinite(value)) {
        if (value <= 0)
            return undefined;
        return value > 10_000_000_000 ? Math.trunc(value) : Math.trunc(value * 1000);
    }
    if (typeof value === 'string' && value.trim()) {
        const trimmed = value.trim();
        if (/^\d+$/.test(trimmed))
            return parseGrokTimestamp(Number.parseInt(trimmed, 10));
        const parsed = Date.parse(trimmed);
        return Number.isFinite(parsed) ? parsed : undefined;
    }
    return undefined;
}
function pickTimestamp(root, paths) {
    for (const path of paths) {
        const parsed = parseGrokTimestamp(getPath(root, path));
        if (parsed != null)
            return parsed;
    }
    return undefined;
}
function capitalize(value) {
    const lower = value.toLowerCase();
    return lower.charAt(0).toUpperCase() + lower.slice(1);
}
function humanizeGrokTier(raw) {
    const trimmed = raw?.trim();
    if (!trimmed)
        return undefined;
    const stripped = trimmed.startsWith('SUBSCRIPTION_TIER_')
        ? trimmed.slice('SUBSCRIPTION_TIER_'.length)
        : trimmed;
    if (!stripped)
        return trimmed;
    return stripped
        .split('_')
        .filter((part) => part.length > 0)
        .map((part) => (part === 'X' ? 'X' : capitalize(part)))
        .join(' ');
}
function humanizeGrokPeriod(raw) {
    const trimmed = raw?.trim();
    if (!trimmed)
        return undefined;
    const stripped = trimmed.startsWith('USAGE_PERIOD_TYPE_')
        ? trimmed.slice('USAGE_PERIOD_TYPE_'.length)
        : trimmed;
    return stripped ? capitalize(stripped) : trimmed;
}
function remainingPercent(usedPercent) {
    return 100 - Math.min(100, Math.max(0, Math.round(usedPercent)));
}
function amountValue(root, paths) {
    for (const path of paths) {
        const value = pickNumber(root, [[...path, 'val']]);
        if (value != null)
            return value;
    }
    return undefined;
}
function billingConfig(raw) {
    if (isRecord(raw) && raw.config != null)
        return raw.config;
    return raw;
}
function readProductUsage(config) {
    const list = isRecord(config) && Array.isArray(config.productUsage) ? config.productUsage : [];
    return list
        .map((item) => {
        const product = pickString(item, [['product']]);
        const usedPercent = pickNumber(item, [['usagePercent']]);
        if (!product || usedPercent == null)
            return undefined;
        return { product, usedPercent, remainingPercent: remainingPercent(usedPercent) };
    })
        .filter((item) => item != null);
}
function buildGrokUsageSummary(credits, history) {
    const creditsConfig = billingConfig(credits);
    const historyConfig = billingConfig(history);
    const periodStartAt = pickTimestamp(creditsConfig, [['currentPeriod', 'start']]);
    const periodResetAt = pickTimestamp(creditsConfig, [['currentPeriod', 'end']])
        ?? pickTimestamp(creditsConfig, [['billingPeriodEnd']]);
    const productUsage = readProductUsage(creditsConfig);
    const creditUsedPercent = pickNumber(creditsConfig, [['creditUsagePercent']])
        ?? productUsage.reduce((highest, item) => (highest == null ? item.usedPercent : Math.max(highest, item.usedPercent)), undefined);
    const monthlyLimit = amountValue(historyConfig, [['monthlyLimit']]);
    return {
        plan: humanizeGrokTier(pickString(creditsConfig, [['subscriptionTier']])),
        creditUsedPercent,
        creditRemainingPercent: creditUsedPercent == null ? undefined : remainingPercent(creditUsedPercent),
        periodLabel: humanizeGrokPeriod(pickString(creditsConfig, [['currentPeriod', 'type']])),
        periodStartAt,
        periodResetAt,
        monthlyUsed: amountValue(historyConfig, [['used']]),
        monthlyLimit: monthlyLimit != null && monthlyLimit > 0 ? monthlyLimit : undefined,
        monthlyPeriodStartAt: pickTimestamp(historyConfig, [['billingPeriodStart']]),
        monthlyPeriodEndAt: pickTimestamp(historyConfig, [['billingPeriodEnd']]),
        onDemandUsed: amountValue(creditsConfig, [['onDemandUsed']]) ?? amountValue(historyConfig, [['onDemandUsed']]),
        onDemandCap: [amountValue(creditsConfig, [['onDemandCap']]), amountValue(historyConfig, [['onDemandCap']])]
            .find((value) => value != null && value > 0) ?? undefined,
        prepaidBalance: amountValue(creditsConfig, [['prepaidBalance']]),
        productUsage,
    };
}
function formatGrokAmount(value) {
    return value == null ? undefined : `$${value.toFixed(2)}`;
}
//# sourceMappingURL=grokUsage.js.map