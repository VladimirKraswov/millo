import { BookOpen, FileCode2, Gauge, Layers3, ScrollText, Settings2, Wrench } from "lucide-react";

interface WorkspaceNavigationProps {
  view: "program" | "controller";
  onView: (view: "program" | "controller") => void;
  onTools: () => void;
  onProbe: () => void;
  onLog: () => void;
  onHelp: () => void;
  onSettings: () => void;
}

export function WorkspaceNavigation(props: WorkspaceNavigationProps) {
  const items = [
    { id: "program", label: "Задание", icon: FileCode2, action: () => props.onView("program"), selected: props.view === "program" },
    { id: "controller", label: "Станок", icon: Gauge, action: () => props.onView("controller"), selected: props.view === "controller" },
    { id: "tools", label: "Фрезы", icon: Wrench, action: props.onTools },
    { id: "probe", label: "Поверхность", icon: Layers3, action: props.onProbe },
    { id: "log", label: "Журнал", icon: ScrollText, action: props.onLog },
    { id: "help", label: "Справка", icon: BookOpen, action: props.onHelp },
    { id: "settings", label: "Настройки", icon: Settings2, action: props.onSettings },
  ];
  return <nav className="workspace-navigation" aria-label="Разделы Millo">
    {items.map(({ id, label, icon: Icon, action, selected }) => <button
      key={id} type="button" onClick={action} aria-label={label} aria-current={selected ? "page" : undefined}
      title={label} className={selected ? "is-selected" : undefined}
    ><Icon aria-hidden="true" size={20} /><span>{label}</span></button>)}
  </nav>;
}
