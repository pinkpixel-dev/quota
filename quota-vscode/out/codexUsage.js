"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.quotaFromUsage = quotaFromUsage;
exports.tracksFromCodexAccount = tracksFromCodexAccount;
// @env node
const constants_1 = require("./constants");
function normalize(value) {
    return typeof value === 'string' && value.trim().length > 0 ? value.trim() : undefined;
}
function clampPercent(value) {
    if (value == null || !Number.isFinite(value))
        return undefined;
    return Math.min(100, Math.max(0, Math.round(value)));
}
function remainingPercent(window) {
    return window == null ? undefined : 100 - Math.min(100, Math.max(0, Math.round(window.used_percent ?? 0)));
}
function windowMinutes(window) {
    const seconds = window?.limit_window_seconds;
    if (seconds == null || seconds <= 0)
        return undefined;
    return Math.ceil(seconds / 60);
}
function resetAt(window, currentTime) {
    if (window?.reset_at != null)
        return window.reset_at * 1000;
    if (window?.reset_after_seconds == null || window.reset_after_seconds < 0)
        return undefined;
    return currentTime + window.reset_after_seconds * 1000;
}
function trackFromAccount(account, id, label) {
    const isPrimary = id === 'codex.primary';
    const remaining = isPrimary ? account.quota.hourlyRemainingPercent : account.quota.weeklyRemainingPercent;
    const reset = isPrimary ? account.quota.hourlyResetAt : account.quota.weeklyResetAt;
    const clampedRemaining = clampPercent(remaining ?? undefined);
    return {
        id,
        providerId: 'codex',
        providerLabel: constants_1.PROVIDER_LABELS.codex,
        label,
        accountLabel: account.email,
        percentUsed: clampedRemaining == null ? undefined : 100 - clampedRemaining,
        percentRemaining: remaining ?? undefined,
        resetAt: reset,
        updatedAt: account.usageUpdatedAt,
        error: account.quotaQueryLastError ?? null,
    };
}
function quotaFromUsage(value, currentTime = Date.now()) {
    const primary = value.rate_limit?.primary_window;
    const secondary = value.rate_limit?.secondary_window;
    return {
        plan: normalize(value.plan_type),
        quota: {
            hourlyRemainingPercent: remainingPercent(primary),
            hourlyResetAt: resetAt(primary, currentTime),
            hourlyWindowMinutes: windowMinutes(primary),
            weeklyRemainingPercent: remainingPercent(secondary),
            weeklyResetAt: resetAt(secondary, currentTime),
            weeklyWindowMinutes: windowMinutes(secondary),
        },
    };
}
function tracksFromCodexAccount(account) {
    return [
        trackFromAccount(account, 'codex.primary', '5h usage'),
        trackFromAccount(account, 'codex.weekly', 'Weekly usage'),
    ].filter((track) => track.percentUsed != null || track.percentRemaining != null || track.error);
}
//# sourceMappingURL=codexUsage.js.map