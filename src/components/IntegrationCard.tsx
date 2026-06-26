import { ArrowRight, ShieldCheck } from 'lucide-react';
import type { IntegrationSummary } from '../data/integrations';

interface IntegrationCardProps {
  integration: IntegrationSummary;
}

export function IntegrationCard({ integration }: IntegrationCardProps) {
  return (
    <article className="integration-card">
      <div className="integration-card__top">
        <span className="integration-card__icon" aria-hidden="true">
          <img src={integration.iconPath} alt="" />
        </span>
        <span className={`integration-card__status integration-card__status--${integration.status}`}>
          {integration.status}
        </span>
      </div>

      <div>
        <h3>{integration.name}</h3>
        <p>{integration.description}</p>
      </div>

      <div className="integration-card__footer">
        <span>
          <ShieldCheck size={15} strokeWidth={1.8} />
          Backend-owned secrets
        </span>
        <ArrowRight size={16} strokeWidth={1.8} />
      </div>
    </article>
  );
}
