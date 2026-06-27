"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.formatPercent = formatPercent;
exports.displayPercent = displayPercent;
exports.statusBarLabel = statusBarLabel;
exports.formatReset = formatReset;
exports.formatUpdated = formatUpdated;
const constants_1 = require("./constants");
function formatPercent(value) {
    return value == null ? 'No data' : `${Math.round(value)}%`;
}
function displayPercent(track, mode) {
    if (mode === 'percentRemaining')
        return track.percentRemaining ?? (track.percentUsed == null ? undefined : 100 - track.percentUsed);
    return track.percentUsed ?? (track.percentRemaining == null ? undefined : 100 - track.percentRemaining);
}
function statusBarLabel(track, mode) {
    const suffix = mode === 'percentRemaining' ? 'left' : 'used';
    const label = constants_1.TRACK_STATUS_BAR_LABEL[track.id] ?? track.providerLabel;
    return `${label} ${formatPercent(displayPercent(track, mode))} ${suffix}`;
}
function formatReset(resetAt) {
    if (resetAt == null)
        return 'Reset unknown';
    const date = new Date(resetAt);
    if (Number.isNaN(date.getTime()))
        return 'Reset unknown';
    return `Resets ${date.toLocaleString(undefined, {
        month: 'short',
        day: 'numeric',
        hour: 'numeric',
        minute: '2-digit',
    })}`;
}
function formatUpdated(updatedAt) {
    if (updatedAt == null)
        return 'Not refreshed yet';
    const date = new Date(updatedAt);
    if (Number.isNaN(date.getTime()))
        return 'Not refreshed yet';
    return `Updated ${date.toLocaleString(undefined, {
        month: 'short',
        day: 'numeric',
        hour: 'numeric',
        minute: '2-digit',
    })}`;
}
//# sourceMappingURL=format.js.map