import { ChevronDown, ExternalLink } from "lucide-react";

import type { CuttingTool } from "../../shared/tooling";
import { toolKnowledge } from "../../shared/tooling";

export function ToolKnowledgePanel({ tool }: { readonly tool: CuttingTool }) {
  const guide = toolKnowledge(tool.kind);
  return (
    <details className="tool-knowledge">
      <summary>
        <span>Описание и рекомендации</span>
        <ChevronDown aria-hidden="true" size={14} />
      </summary>
      <div className="tool-knowledge-body">
        <p>{tool.description}</p>
        <section>
          <strong>Лучше всего</strong>
          <ul>{guide.bestFor.map((item) => <li key={item}>{item}</li>)}</ul>
        </section>
        <section className="is-caution">
          <strong>Обратите внимание</strong>
          <ul>{guide.cautions.map((item) => <li key={item}>{item}</li>)}</ul>
        </section>
        {tool.reference && (
          <a href={tool.reference.url} rel="noreferrer" target="_blank">
            {tool.reference.manufacturer} · {tool.reference.product}
            <ExternalLink aria-hidden="true" size={12} />
          </a>
        )}
      </div>
    </details>
  );
}
