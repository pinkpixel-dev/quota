"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.parseAntigravityLoadStatus = parseAntigravityLoadStatus;
exports.formatAntigravityCredits = formatAntigravityCredits;
exports.buildAntigravityCreditsTrack = buildAntigravityCreditsTrack;
const constants_1 = require("./constants");
function normalize(value) {
    return typeof value === 'string' && value.trim().length > 0 ? value.trim() : undefined;
}
function readPath(root, path) {
    let current = root;
    for (const key of path) {
        if (current == null || typeof current !== 'object')
            return undefined;
        current = current[key];
    }
    return current;
}
function readString(root, path) {
    return normalize(readPath(root, path));
}
function readArray(root, path) {
    const value = readPath(root, path);
    return Array.isArray(value) ? value : [];
}
function firstString(values) {
    return values.find((value) => value != null);
}
function creditNumber(value) {
    if (value == null)
        return undefined;
    const parsed = Number.parseFloat(value.replace(/,/g, '').trim());
    return Number.isFinite(parsed) ? parsed : undefined;
}
function parseAntigravityLoadStatus(raw) {
    const paidTier = readPath(raw, ['paidTier']);
    const currentTier = readPath(raw, ['currentTier']);
    const projectValue = readPath(raw, ['cloudaicompanionProject']);
    const credits = readArray(paidTier, ['availableCredits'])
        .map((item) => {
        const creditType = readString(item, ['creditType']);
        const creditAmount = readString(item, ['creditAmount']);
        if (!creditType || !creditAmount)
            return undefined;
        return {
            creditType,
            creditAmount,
            minimumCreditAmountForUsage: readString(item, ['minimumCreditAmountForUsage']),
        };
    })
        .filter((item) => item != null);
    return {
        tierId: firstString([
            readString(paidTier, ['id']),
            readString(currentTier, ['id']),
            readString(readArray(raw, ['allowedTiers'])[0], ['id']),
        ]),
        tierName: firstString([
            readString(paidTier, ['name']),
            readString(currentTier, ['name']),
        ]),
        projectId: normalize(projectValue)
            ?? readString(projectValue, ['id'])
            ?? readString(projectValue, ['projectId']),
        credits,
    };
}
function formatAntigravityCredits(credits) {
    let total = 0;
    let hasValidAmount = false;
    for (const credit of credits ?? []) {
        const parsed = creditNumber(credit.creditAmount);
        if (parsed == null)
            continue;
        total += parsed;
        hasValidAmount = true;
    }
    return hasValidAmount
        ? total.toLocaleString(undefined, { maximumFractionDigits: 2 })
        : undefined;
}
function buildAntigravityCreditsTrack(account) {
    const valueLabel = formatAntigravityCredits(account.credits);
    if (!valueLabel)
        return undefined;
    return {
        id: 'antigravity.credits',
        providerId: 'antigravity',
        providerLabel: constants_1.PROVIDER_LABELS.antigravity,
        label: 'Available AI Credits',
        accountLabel: account.name ? `${account.name} (${account.email})` : account.email,
        valueLabel,
        updatedAt: account.usageUpdatedAt,
        error: account.quotaQueryLastError ?? null,
    };
}
//# sourceMappingURL=antigravityUsage.js.map