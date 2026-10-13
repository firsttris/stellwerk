import type { Device } from "../api/types";
import { FolderPicker } from "./folder-picker";
import { Input } from "./ui/input";

/** Where a folder lives on a device: typed, or picked with the device's folder picker. */
export function PathField({
  device,
  value,
  onChange,
  onPick = onChange,
  placeholder,
  label,
  id,
  disabled,
}: {
  device: Device;
  value: string;
  onChange: (path: string) => void;
  /** What a pick from the picker does; typing the same by default. */
  onPick?: (path: string) => void;
  placeholder: string;
  label?: string;
  id?: string;
  disabled?: boolean;
}) {
  return (
    <div className="flex gap-2">
      <Input
        id={id}
        aria-label={label}
        className="font-mono"
        value={value}
        placeholder={placeholder}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value)}
      />
      <FolderPicker device={device} value={value} disabled={disabled} onPick={onPick} />
    </div>
  );
}
