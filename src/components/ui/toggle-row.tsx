import { Row } from "@/components/ui/group";
import { Switch } from "@/components/ui/switch";

export interface ToggleRowProps {
  icon?: React.ReactNode;
  title: string;
  description?: string;
  checked: boolean;
  onCheckedChange: (value: boolean) => void;
  disabled?: boolean;
}

/**
 * A grouped-list row whose trailing control is a switch. Inside a `Group` the
 * card and separators come from the group, so this draws neither.
 */
export function ToggleRow({
  icon,
  title,
  description,
  checked,
  onCheckedChange,
  disabled,
}: ToggleRowProps) {
  return (
    <Row
      icon={icon}
      label={title}
      description={description}
      disabled={disabled}
      trailing={
        <Switch
          checked={checked}
          onCheckedChange={onCheckedChange}
          disabled={disabled}
          aria-label={title}
        />
      }
    />
  );
}
