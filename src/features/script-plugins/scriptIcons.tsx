import {
  ArrowUpFromLine,
  Braces,
  CircleHelp,
  Crosshair,
  LocateFixed,
  ScanLine,
  type LucideIcon,
} from "lucide-react";

const icons: Readonly<Record<string, LucideIcon>> = {
  "arrow-up-from-line": ArrowUpFromLine,
  braces: Braces,
  crosshair: Crosshair,
  "locate-fixed": LocateFixed,
  "scan-line": ScanLine,
};

export const scriptIcon = (name: string): LucideIcon => icons[name] ?? CircleHelp;
