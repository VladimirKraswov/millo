import { FileUp, Replace } from "lucide-react";
import type { ChangeEvent } from "react";

const acceptedProgramTypes = ".nc,.ngc,.gcode,.tap,.cnc";

interface ProgramFilePickerProps {
  readonly disabled: boolean;
  readonly loading: boolean;
  readonly onSelect: (file: File) => void;
  readonly variant: "empty" | "toolbar";
}

export function ProgramFilePicker({
  disabled,
  loading,
  onSelect,
  variant,
}: ProgramFilePickerProps) {
  const selectFile = (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.currentTarget.files?.[0];
    event.currentTarget.value = "";
    if (file) onSelect(file);
  };
  const empty = variant === "empty";
  const label = loading ? "Разбор файла..." : empty ? "Открыть G-code" : "Заменить файл";
  const Icon = empty ? FileUp : Replace;

  return (
    <label
      className={`program-file-picker is-${variant}${loading ? " is-loading" : ""}`}
    >
      <Icon aria-hidden="true" size={empty ? 18 : 14} />
      <span>{label}</span>
      <input
        accept={acceptedProgramTypes}
        aria-label={label}
        disabled={disabled || loading}
        onChange={selectFile}
        type="file"
      />
    </label>
  );
}
