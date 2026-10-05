import { ExternalLink } from 'lucide-react';
import { useState } from 'react';
import type { FormEvent } from 'react';

const OPENCODE_CONSOLE_URL = 'https://opencode.ai/auth';

interface OpenCodeGoKeyFormProps {
  busy: boolean;
  onOpenConsole: (url: string) => void;
  onSubmit: (apiKey: string, name: string) => Promise<boolean>;
  onCancel: () => void;
}

export function OpenCodeGoKeyForm({ busy, onOpenConsole, onSubmit, onCancel }: OpenCodeGoKeyFormProps) {
  const [apiKey, setApiKey] = useState('');
  const [name, setName] = useState('');

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!apiKey.trim() || busy) return;
    if (await onSubmit(apiKey.trim(), name.trim())) {
      setApiKey('');
      setName('');
    }
  }

  return (
    <form className="auth-panel" onSubmit={handleSubmit}>
      <div>
        <span className="auth-panel__label">OpenCode Go API key</span>
        <strong>OpenCode Go</strong>
      </div>
      <p>Paste the API key from your OpenCode console. Quota checks it before saving.</p>
      <div className="auth-panel__fields">
        <label>
          <span>API key</span>
          <input
            type="password"
            value={apiKey}
            onChange={(event) => setApiKey(event.currentTarget.value)}
            autoComplete="off"
            spellCheck={false}
            required
            autoFocus
          />
        </label>
        <label>
          <span>Name</span>
          <input
            type="text"
            value={name}
            onChange={(event) => setName(event.currentTarget.value)}
            placeholder="Optional"
            autoComplete="off"
          />
        </label>
      </div>
      <div className="button-row">
        <button type="button" onClick={() => onOpenConsole(OPENCODE_CONSOLE_URL)}>
          <ExternalLink size={15} />
          Open console
        </button>
        <button type="submit" className="button-primary" disabled={busy || apiKey.trim().length === 0}>
          {busy ? 'Checking key…' : 'Add key'}
        </button>
        <button type="button" onClick={onCancel} disabled={busy}>
          Cancel
        </button>
      </div>
    </form>
  );
}
