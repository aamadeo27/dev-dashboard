// LaunchModal — see ui-ux-spec.md §6 and KB §4
// Props contract defined here; full implementation is T5.8.
export interface LaunchModalProps {
  projectId: string;
  sequenceName: string;
  onClose: () => void;
}

export default function LaunchModal(_props: LaunchModalProps) {
  return null;
}
