// Simple line icons drawn for this app.

import type { ReactNode } from "react";

type IconProps = { className?: string };

export function CheckIcon({ className }: IconProps) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3.2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M4 12.5l5 5L20 6.5" />
    </svg>
  );
}

export function CrossIcon({ className }: IconProps) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3.2" strokeLinecap="round">
      <path d="M6 6l12 12M18 6L6 18" />
    </svg>
  );
}

export function WarningIcon({ className }: IconProps) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M12 3.5L22 20.5H2z" />
      <path d="M12 10v4.5" />
      <circle cx="12" cy="17.5" r="0.6" fill="currentColor" />
    </svg>
  );
}

export function QuestionIcon({ className }: IconProps) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.6" strokeLinecap="round">
      <path d="M9 9a3 3 0 1 1 4.2 2.75c-.75.33-1.2 1.08-1.2 1.9V14" />
      <circle cx="12" cy="18" r="0.6" fill="currentColor" />
    </svg>
  );
}

export function PeopleIcon({ className }: IconProps) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="currentColor">
      <circle cx="12" cy="7.5" r="3.2" />
      <path d="M5.5 20c0-3.6 2.9-6 6.5-6s6.5 2.4 6.5 6z" />
      <circle cx="5" cy="9" r="2.2" opacity="0.7" />
      <circle cx="19" cy="9" r="2.2" opacity="0.7" />
      <path d="M0.5 18.5c0-2.6 1.8-4.4 4.2-4.4 0.8 0 1.4 0.2 2 0.5-1.2 1.1-1.9 2.4-2.1 3.9z" opacity="0.7" />
      <path d="M23.5 18.5c0-2.6-1.8-4.4-4.2-4.4-0.8 0-1.4 0.2-2 0.5 1.2 1.1 1.9 2.4 2.1 3.9z" opacity="0.7" />
    </svg>
  );
}

export function TrophyIcon({ className }: IconProps) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="currentColor">
      <path d="M7 3h10v5a5 5 0 0 1-10 0z" />
      <path d="M7 4.5H3.5v1.5a4 4 0 0 0 4 4M17 4.5h3.5v1.5a4 4 0 0 1-4 4" fill="none" stroke="currentColor" strokeWidth="1.6" />
      <path d="M10.8 13h2.4v3.5h-2.4z" />
      <path d="M7.5 21v-1.6c0-1.2 1-2.1 2.1-2.1h4.8c1.2 0 2.1 1 2.1 2.1V21z" />
    </svg>
  );
}

export function NextIcon({ className }: IconProps) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round">
      <path d="M5 12h13M13 6l6 6-6 6" />
    </svg>
  );
}

export function PauseIcon({ className }: IconProps) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="currentColor">
      <rect x="6" y="5" width="4" height="14" rx="1" />
      <rect x="14" y="5" width="4" height="14" rx="1" />
    </svg>
  );
}

export function PlayIcon({ className }: IconProps) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="currentColor">
      <path d="M8 5.5v13l10.5-6.5z" />
    </svg>
  );
}

export function StopIcon({ className }: IconProps) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="currentColor">
      <rect x="6.5" y="6.5" width="11" height="11" rx="1.5" />
    </svg>
  );
}

export function ListIcon({ className }: IconProps) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round">
      <path d="M9 6.5h11M9 12h11M9 17.5h11" />
      <circle cx="4.5" cy="6.5" r="1" fill="currentColor" />
      <circle cx="4.5" cy="12" r="1" fill="currentColor" />
      <circle cx="4.5" cy="17.5" r="1" fill="currentColor" />
    </svg>
  );
}

export function ScreenIcon({ className }: IconProps) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinejoin="round">
      <rect x="3" y="4.5" width="18" height="12" rx="1.5" />
      <path d="M9 20h6M12 16.5V20" />
    </svg>
  );
}

export function FolderIcon({ className }: IconProps) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinejoin="round">
      <path d="M3 6.5a1.5 1.5 0 0 1 1.5-1.5H9l2 2.5h8.5A1.5 1.5 0 0 1 21 9v9.5a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 18.5z" />
    </svg>
  );
}

export function PowerIcon({ className }: IconProps) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round">
      <path d="M12 3.5v8" />
      <path d="M7 6.5a7.5 7.5 0 1 0 10 0" />
    </svg>
  );
}

export function SoundIcon({ className }: IconProps) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinejoin="round" strokeLinecap="round">
      <path d="M4 9.5h3.5L12 5.5v13l-4.5-4H4z" />
      <path d="M15.5 9a4 4 0 0 1 0 6M18 6.5a7.5 7.5 0 0 1 0 11" />
    </svg>
  );
}

export function EditIcon({ className }: IconProps) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M4 20h4L19 9l-4-4L4 16v4z" />
      <path d="M13.5 6.5l4 4" />
    </svg>
  );
}

// Editor ribbon. Stroke icons at 1.8 width, drawn for this app.

function LineIcon({ className, children }: IconProps & { children: ReactNode }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      {children}
    </svg>
  );
}

export function SaveIcon(props: IconProps) {
  return (
    <LineIcon {...props}>
      <path d="M5 4h11l3 3v13H5z" />
      <path d="M8 4v5h7V4M8 20v-6h8v6" />
    </LineIcon>
  );
}

export function UndoIcon(props: IconProps) {
  return (
    <LineIcon {...props}>
      <path d="M9 14L4 9l5-5" />
      <path d="M4 9h11a5 5 0 0 1 0 10h-3" />
    </LineIcon>
  );
}

export function RedoIcon(props: IconProps) {
  return (
    <LineIcon {...props}>
      <path d="M15 14l5-5-5-5" />
      <path d="M20 9H9a5 5 0 0 0 0 10h3" />
    </LineIcon>
  );
}

export function SlideAddIcon(props: IconProps) {
  return (
    <LineIcon {...props}>
      <rect x="3" y="5" width="14" height="10" rx="1" />
      <path d="M19 15v6M16 18h6" />
    </LineIcon>
  );
}

export function PictureIcon(props: IconProps) {
  return (
    <LineIcon {...props}>
      <rect x="3" y="5" width="18" height="14" rx="1.5" />
      <circle cx="8.5" cy="10" r="1.5" />
      <path d="M21 16l-5-5-8 8" />
    </LineIcon>
  );
}

export function VideoIcon(props: IconProps) {
  return (
    <LineIcon {...props}>
      <rect x="3" y="6" width="13" height="12" rx="1.5" />
      <path d="M16 10l5-3v10l-5-3z" />
    </LineIcon>
  );
}

export function TrimIcon(props: IconProps) {
  return (
    <LineIcon {...props}>
      <path d="M2 12h2M6 8v8M9 5v14M12 9v6M15 6v12M18 10v4M20 12h2" />
      <path d="M7.5 3v18M16.5 3v18" strokeDasharray="2 2" />
    </LineIcon>
  );
}

export function LoudnessIcon(props: IconProps) {
  return (
    <LineIcon {...props}>
      <path d="M4 20V14M9 20V9M14 20V12M19 20V5" />
      <path d="M2 10h20" strokeDasharray="2 2" />
    </LineIcon>
  );
}

export function BackgroundIcon(props: IconProps) {
  return (
    <LineIcon {...props}>
      <rect x="3" y="4" width="18" height="16" rx="1.5" />
      <path d="M3 15l5-4 4 3 3-2 6 4" />
      <rect x="8" y="7" width="8" height="5" rx="0.5" />
    </LineIcon>
  );
}

export function CopyIcon(props: IconProps) {
  return (
    <LineIcon {...props}>
      <rect x="8" y="8" width="12" height="12" rx="1.5" />
      <path d="M16 8V5a1 1 0 0 0-1-1H5a1 1 0 0 0-1 1v10a1 1 0 0 0 1 1h3" />
    </LineIcon>
  );
}

export function CutIcon(props: IconProps) {
  return (
    <LineIcon {...props}>
      <circle cx="6" cy="18" r="2.5" />
      <circle cx="18" cy="18" r="2.5" />
      <path d="M8 16L18 4M16 16L6 4" />
    </LineIcon>
  );
}

export function PasteIcon(props: IconProps) {
  return (
    <LineIcon {...props}>
      <path d="M9 4h6v3H9z" />
      <path d="M15 5h3v15H6V5h3" />
      <path d="M9 12h6M9 16h4" />
    </LineIcon>
  );
}

export function TrashIcon(props: IconProps) {
  return (
    <LineIcon {...props}>
      <path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13" />
      <path d="M10 11v6M14 11v6" />
    </LineIcon>
  );
}

export function ArrowUpIcon(props: IconProps) {
  return (
    <LineIcon {...props}>
      <path d="M12 19V5M6 11l6-6 6 6" />
    </LineIcon>
  );
}

export function ArrowDownIcon(props: IconProps) {
  return (
    <LineIcon {...props}>
      <path d="M12 5v14M6 13l6 6 6-6" />
    </LineIcon>
  );
}

export function TableIcon(props: IconProps) {
  return (
    <LineIcon {...props}>
      <rect x="3" y="5" width="18" height="14" rx="1" />
      <path d="M3 10h18M3 15h18M9 5v14" />
    </LineIcon>
  );
}

export function SlideIcon(props: IconProps) {
  return (
    <LineIcon {...props}>
      <rect x="3" y="5" width="18" height="14" rx="1" />
      <path d="M7 9h10M8 13h3M13 13h3M8 16h3M13 16h3" />
    </LineIcon>
  );
}

export function PanelIcon(props: IconProps) {
  return (
    <LineIcon {...props}>
      <rect x="3" y="4" width="18" height="16" rx="1" />
      <path d="M15 4v16M17 8h2M17 11h2M17 14h2" />
    </LineIcon>
  );
}

export function NotesIcon(props: IconProps) {
  return (
    <LineIcon {...props}>
      <rect x="3" y="4" width="18" height="16" rx="1" />
      <path d="M3 14h18M6 17h8" />
    </LineIcon>
  );
}

export function PreviewIcon(props: IconProps) {
  return (
    <LineIcon {...props}>
      <path d="M4 8V4h4M20 8V4h-4M4 16v4h4M20 16v4h-4" />
      <path d="M10 9l5 3-5 3z" />
    </LineIcon>
  );
}

export function BackIcon(props: IconProps) {
  return (
    <LineIcon {...props}>
      <circle cx="12" cy="12" r="9" />
      <path d="M13 8l-4 4 4 4M9 12h7" />
    </LineIcon>
  );
}

export function GamepadIcon(props: IconProps) {
  return (
    <LineIcon {...props}>
      <path d="M7 7h10a5 5 0 0 1 0 10c-1.6 0-2.4-1.5-3.5-1.5h-3C9.4 15.5 8.6 17 7 17A5 5 0 0 1 7 7z" />
      <path d="M7 10v4M5 12h4M15.5 11h.01M17.5 13h.01" />
    </LineIcon>
  );
}

export function StarIcon(props: IconProps) {
  return (
    <LineIcon {...props}>
      <path d="M12 3.5l2.6 5.3 5.9.9-4.3 4.1 1 5.8L12 16.9l-5.2 2.7 1-5.8-4.3-4.1 5.9-.9z" />
    </LineIcon>
  );
}

export function PaletteIcon(props: IconProps) {
  return (
    <LineIcon {...props}>
      <path d="M12 3.5a8.5 8.5 0 0 0 0 17c1.2 0 1.8-.8 1.8-1.7 0-1.3-1.2-1.6-1.2-2.8 0-1 .8-1.6 1.8-1.6h2.2a3.9 3.9 0 0 0 3.9-3.9c0-4-3.8-7-8.5-7z" />
      <path d="M7.5 12h.01M9 8h.01M13 6.5h.01M16.5 9h.01" />
    </LineIcon>
  );
}

/** A tag: a category of slides. */
export function CategoryIcon(props: IconProps) {
  return (
    <LineIcon {...props}>
      <path d="M3.5 12.2V4.5a1 1 0 0 1 1-1h7.7l8.3 8.3a1 1 0 0 1 0 1.4l-7 7a1 1 0 0 1-1.4 0z" />
      <path d="M8 8h.01" />
    </LineIcon>
  );
}

export function ChevronLeftIcon(props: IconProps) {
  return (
    <LineIcon {...props}>
      <path d="M14.5 5.5 8 12l6.5 6.5" />
    </LineIcon>
  );
}

export function ChevronRightIcon(props: IconProps) {
  return (
    <LineIcon {...props}>
      <path d="M9.5 5.5 16 12l-6.5 6.5" />
    </LineIcon>
  );
}

/** A stopwatch: a question's clock. */
export function TimerIcon(props: IconProps) {
  return (
    <LineIcon {...props}>
      <circle cx="12" cy="13.5" r="7" />
      <path d="M10 3h4M12 3v3.5M12 13.5V10M18 6.5l1.5-1.5" />
    </LineIcon>
  );
}

export function AddIcon(props: IconProps) {
  return (
    <LineIcon {...props}>
      <path d="M12 5v14M5 12h14" />
    </LineIcon>
  );
}

export function RewindIcon(props: IconProps) {
  return (
    <LineIcon {...props}>
      <path d="M6 6v12M18 6l-8 6 8 6z" />
    </LineIcon>
  );
}

export function GridIcon(props: IconProps) {
  return (
    <LineIcon {...props}>
      <rect x="3.5" y="3.5" width="7" height="7" rx="1" />
      <rect x="13.5" y="3.5" width="7" height="7" rx="1" />
      <rect x="3.5" y="13.5" width="7" height="7" rx="1" />
      <rect x="13.5" y="13.5" width="7" height="7" rx="1" />
    </LineIcon>
  );
}
