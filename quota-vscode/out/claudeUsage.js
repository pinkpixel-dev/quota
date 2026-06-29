"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.shouldSuppressClaudeError = shouldSuppressClaudeError;
function shouldSuppressClaudeError(message) {
    const lower = message.toLowerCase();
    return lower.includes('claude usage returned 429')
        || lower.includes('claude usage is rate limited')
        || lower.includes('rate_limit');
}
//# sourceMappingURL=claudeUsage.js.map