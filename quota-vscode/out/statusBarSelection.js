"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.isTrackId = isTrackId;
exports.parseStatusBarEntry = parseStatusBarEntry;
exports.statusBarEntryKey = statusBarEntryKey;
exports.shortAccountTag = shortAccountTag;
exports.isTrackSelected = isTrackSelected;
exports.resolveStatusBarTracks = resolveStatusBarTracks;
const constants_1 = require("./constants");
const ACCOUNT_TAG_MAX_LENGTH = 12;
function isTrackId(value) {
    return constants_1.CANONICAL_TRACK_ORDER.includes(value);
}
function parseStatusBarEntry(value) {
    const separator = value.indexOf('@');
    const trackId = separator === -1 ? value : value.slice(0, separator);
    const accountId = separator === -1 ? undefined : value.slice(separator + 1).trim();
    if (!isTrackId(trackId))
        return undefined;
    if (accountId === '')
        return undefined;
    return accountId ? { trackId, accountId } : { trackId };
}
function statusBarEntryKey(track) {
    return `${track.id}@${track.accountId}`;
}
function canonicalIndex(id) {
    const index = constants_1.CANONICAL_TRACK_ORDER.indexOf(id);
    return index === -1 ? 999 : index;
}
function findTrack(entry, tracks) {
    return tracks.find((track) => (track.id === entry.trackId && (entry.accountId == null || track.accountId === entry.accountId)));
}
/** Account IDs in the order their provider first reports them. */
function accountOrder(tracks, providerId) {
    const ids = [];
    for (const track of tracks) {
        if (track.providerId === providerId && !ids.includes(track.accountId))
            ids.push(track.accountId);
    }
    return ids;
}
/** Short account name for the status bar, e.g. `work` from `work@example.com`. */
function shortAccountTag(accountLabel) {
    const email = accountLabel.match(/[^\s(]+@[^\s)]+/)?.[0];
    const base = email ? email.slice(0, email.indexOf('@')) : accountLabel.trim();
    return base.length > ACCOUNT_TAG_MAX_LENGTH ? `${base.slice(0, ACCOUNT_TAG_MAX_LENGTH - 1)}…` : base;
}
/** True when an entry selects this exact track, including legacy bare IDs that resolve to it. */
function isTrackSelected(entries, track, tracks) {
    return entries.some((value) => {
        const entry = parseStatusBarEntry(value);
        if (!entry)
            return false;
        return findTrack(entry, tracks) === track;
    });
}
function resolveStatusBarTracks(entries, tracks, enabledProviders, maxItems) {
    const enabledTracks = tracks.filter((track) => enabledProviders.includes(track.providerId));
    const resolved = new Map();
    for (const value of entries) {
        const entry = parseStatusBarEntry(value);
        if (!entry)
            continue;
        const track = findTrack(entry, enabledTracks);
        if (track)
            resolved.set(statusBarEntryKey(track), track);
    }
    const withOrder = [...resolved.entries()].map(([key, track]) => {
        const accounts = accountOrder(enabledTracks, track.providerId);
        return {
            key,
            track,
            accountIndex: accounts.indexOf(track.accountId),
            accountCount: accounts.length,
        };
    });
    withOrder.sort((a, b) => (constants_1.PROVIDER_ORDER.indexOf(a.track.providerId) - constants_1.PROVIDER_ORDER.indexOf(b.track.providerId)
        || a.accountIndex - b.accountIndex
        || canonicalIndex(a.track.id) - canonicalIndex(b.track.id)));
    return withOrder.slice(0, Math.max(0, maxItems)).map((item, index) => ({
        key: item.key,
        track: item.track,
        accountTag: item.accountCount > 1 ? shortAccountTag(item.track.accountLabel) : undefined,
        // Left-to-right order in the right-aligned status bar: higher priority sits further left.
        priority: 89 - index,
    }));
}
//# sourceMappingURL=statusBarSelection.js.map