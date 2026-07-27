import { KeyRound } from 'lucide-react';

interface ReauthenticationAlertProps {
  message: string;
  busy: boolean;
  onReauthenticate: () => void;
}

export function ReauthenticationAlert({
  message,
  busy,
  onReauthenticate,
}: ReauthenticationAlertProps) {
  return (
    <div className="reauthentication-alert" role="alert">
      <div>
        <strong>Reauthentication required</strong>
        <p>{message}</p>
      </div>
      <button type="button" onClick={onReauthenticate} disabled={busy}>
        <KeyRound size={14} />
        Reauthenticate
      </button>
    </div>
  );
}
