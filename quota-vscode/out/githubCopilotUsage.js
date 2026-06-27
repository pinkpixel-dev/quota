"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.parseCopilotResetDate = parseCopilotResetDate;
exports.buildCopilotUsageSummary = buildCopilotUsageSummary;
function asRecord(value) {
    return typeof value === 'object' && value !== null && !Array.isArray(value) ? value : undefined;
}
function getNumber(value) {
    if (typeof value === 'number' && Number.isFinite(value))
        return Math.round(value);
    if (typeof value === 'string') {
        const parsed = Number.parseFloat(value);
        if (Number.isFinite(parsed))
            return Math.round(parsed);
    }
    return undefined;
}
function clampPercent(value) {
    if (value == null || !Number.isFinite(value))
        return undefined;
    return Math.min(100, Math.max(0, Math.round(value)));
}
function snapshot(input, key) {
    return asRecord(asRecord(input)?.[key]);
}
function limitedQuota(input, key) {
    return getNumber(asRecord(input)?.[key]);
}
function tokenValue(token, key) {
    const prefix = token.split(':')[0] ?? token;
    for (const part of prefix.split(';')) {
        const [partKey, partValue] = part.split('=');
        if (partKey?.trim() === key)
            return partValue?.trim();
    }
    return undefined;
}
function resetFromToken(token) {
    const value = tokenValue(token, 'rd');
    const head = value?.split(':')[0]?.trim();
    const parsed = head == null ? undefined : Number.parseInt(head, 10);
    return parsed != null && Number.isFinite(parsed) ? parsed * 1000 : undefined;
}
function parseCopilotResetDate(date) {
    const trimmed = date.trim();
    if (!trimmed)
        return undefined;
    if (/^\d{4}-\d{2}-\d{2}$/.test(trimmed)) {
        const parsed = Date.parse(`${trimmed}T00:00:00Z`);
        return Number.isFinite(parsed) ? parsed : undefined;
    }
    const parsed = Date.parse(trimmed);
    return Number.isFinite(parsed) ? parsed : undefined;
}
function percentFromSnapshot(item) {
    if (!item)
        return undefined;
    if (item.unlimited === true)
        return 0;
    const entitlement = getNumber(item.entitlement);
    if (entitlement != null && entitlement < 0)
        return 0;
    const percentRemaining = getNumber(item.percent_remaining);
    return percentRemaining == null ? undefined : Math.min(100, Math.max(0, 100 - percentRemaining));
}
function includedFromSnapshot(item) {
    if (!item)
        return false;
    if (item.unlimited === true)
        return true;
    const entitlement = getNumber(item.entitlement);
    return entitlement != null && entitlement < 0;
}
function remainingFromSnapshot(item) {
    if (!item)
        return undefined;
    const remaining = getNumber(item.remaining);
    if (remaining != null)
        return remaining;
    const entitlement = getNumber(item.entitlement);
    const percentRemaining = getNumber(item.percent_remaining);
    if (entitlement == null || percentRemaining == null || entitlement <= 0)
        return undefined;
    return Math.round((entitlement * percentRemaining) / 100);
}
function usedPercent(total, remaining) {
    if (total == null || remaining == null || total <= 0)
        return undefined;
    return clampPercent(((total - remaining) / total) * 100);
}
function buildCopilotUsageSummary(input) {
    const completionsSnapshot = snapshot(input.quotaSnapshots, 'completions');
    const chatSnapshot = snapshot(input.quotaSnapshots, 'chat');
    const premiumSnapshot = snapshot(input.quotaSnapshots, 'premium_interactions') ?? snapshot(input.quotaSnapshots, 'premium_models');
    const remainingCompletions = remainingFromSnapshot(completionsSnapshot) ?? limitedQuota(input.limitedUserQuotas, 'completions');
    const remainingChat = remainingFromSnapshot(chatSnapshot) ?? limitedQuota(input.limitedUserQuotas, 'chat');
    const remainingPremium = remainingFromSnapshot(premiumSnapshot);
    const totalCompletions = getNumber(completionsSnapshot?.entitlement) ?? remainingCompletions;
    const totalChat = getNumber(chatSnapshot?.entitlement) ?? remainingChat;
    const totalPremium = getNumber(premiumSnapshot?.entitlement) ?? remainingPremium;
    const exactRemainingPremium = getNumber(premiumSnapshot?.remaining);
    const resetFromDate = input.quotaResetDate ? parseCopilotResetDate(input.quotaResetDate) : undefined;
    const resetFromLimited = input.limitedUserResetDate != null ? input.limitedUserResetDate * 1000 : undefined;
    return {
        inlineSuggestionsUsedPercent: percentFromSnapshot(completionsSnapshot) ?? usedPercent(totalCompletions, remainingCompletions),
        chatMessagesUsedPercent: percentFromSnapshot(chatSnapshot) ?? usedPercent(totalChat, remainingChat),
        premiumRequestsUsedPercent: percentFromSnapshot(premiumSnapshot) ?? usedPercent(totalPremium, remainingPremium),
        inlineIncluded: includedFromSnapshot(completionsSnapshot),
        chatIncluded: includedFromSnapshot(chatSnapshot),
        premiumIncluded: includedFromSnapshot(premiumSnapshot),
        remainingCompletions,
        remainingChat,
        remainingPremiumRequests: exactRemainingPremium ?? remainingPremium,
        totalCompletions,
        totalChat,
        totalPremiumRequests: totalPremium,
        usedPremiumRequests: totalPremium != null && exactRemainingPremium != null ? Math.max(0, totalPremium - exactRemainingPremium) : undefined,
        allowanceResetAt: resetFromLimited ?? resetFromDate ?? resetFromToken(input.copilotToken),
    };
}
//# sourceMappingURL=githubCopilotUsage.js.map