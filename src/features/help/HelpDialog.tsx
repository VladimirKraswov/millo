import { useEffect, useRef, useState } from "react";
import { BookOpen, Search, X } from "lucide-react";
import { searchHelp } from "./helpTopics";

export function HelpDialog({ onClose }: { onClose: () => void }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState("first-job");
  useEffect(() => {
    const previous = document.activeElement;
    dialog.current?.querySelector<HTMLButtonElement>("button")?.focus();
    return () => { if (previous instanceof HTMLElement && previous.isConnected) previous.focus(); };
  }, []);
  const results = searchHelp(query);
  const topic = results.find((entry) => entry.id === selected) ?? results[0];
  return <dialog ref={dialog} open className="help-dialog" aria-modal="false" aria-labelledby="help-title" onKeyDown={(event) => { if (event.key === "Escape") { event.stopPropagation(); onClose(); } }}>
    <header><BookOpen size={20} /><div><span>Millo</span><h2 id="help-title">Справка по работе</h2></div><button onClick={onClose} title="Закрыть справку" aria-label="Закрыть справку" type="button"><X size={20} /></button></header>
    <label className="help-search"><Search size={17} /><input aria-label="Поиск по справке" placeholder="Ноль, карта высот, остановка…" value={query} onChange={(event) => setQuery(event.target.value)} /></label>
    <div className="help-body"><nav aria-label="Темы справки">{results.map((entry) => <button key={entry.id} aria-current={topic?.id === entry.id ? "page" : undefined} onClick={() => setSelected(entry.id)} type="button"><small>{entry.category}</small><strong>{entry.title}</strong></button>)}</nav>
      <article>{topic ? <><span>{topic.category}</span><h3>{topic.title}</h3><p className="help-summary">{topic.summary}</p>{topic.paragraphs.map((paragraph) => <p key={paragraph}>{paragraph}</p>)}</> : <><h3>Ничего не найдено</h3><p>Попробуйте другое слово.</p></>}</article>
    </div>
  </dialog>;
}
