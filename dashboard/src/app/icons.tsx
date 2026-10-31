import type { SVGProps } from 'react';

/** One outline icon set, drawn in currentColor so state comes from CSS. 1.5px beside regular text. */
function Icon({ children, ...rest }: SVGProps<SVGSVGElement>) {
  return (
    <svg
      width="20"
      height="20"
      viewBox="0 0 20 20"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
      {...rest}
    >
      {children}
    </svg>
  );
}

export const GearIcon = () => (
  <Icon>
    <circle cx="10" cy="10" r="2.5" />
    <path d="M10 2.5v2M10 15.5v2M2.5 10h2M15.5 10h2M4.7 4.7l1.4 1.4M13.9 13.9l1.4 1.4M4.7 15.3l1.4-1.4M13.9 6.1l1.4-1.4" />
  </Icon>
);
export const CloseIcon = () => (
  <Icon>
    <path d="M5 5l10 10M15 5L5 15" />
  </Icon>
);
export const MoveIcon = () => (
  <Icon>
    <path d="M10 2.5v15M2.5 10h15M10 2.5l-2 2M10 2.5l2 2M10 17.5l-2-2M10 17.5l2-2M2.5 10l2-2M2.5 10l2 2M17.5 10l-2-2M17.5 10l-2 2" />
  </Icon>
);
export const ChevronIcon = () => (
  <Icon>
    <path d="M5 8l5 5 5-5" />
  </Icon>
);
export const CopyIcon = () => (
  <Icon>
    <rect x="7" y="7" width="10" height="10" rx="1.5" />
    <path d="M13 7V4.5A1.5 1.5 0 0 0 11.5 3h-7A1.5 1.5 0 0 0 3 4.5v7A1.5 1.5 0 0 0 4.5 13H7" />
  </Icon>
);
export const RenameIcon = () => (
  <Icon>
    <path d="M3.5 16.5l1-4L14 3l2.5 2.5L7 15z" />
    <path d="M12.5 4.5L15 7" />
  </Icon>
);
export const TrashIcon = () => (
  <Icon>
    <path d="M4 6h12M8 6V4h4v2M6 6l.7 10h6.6L14 6" />
  </Icon>
);
export const PlusIcon = () => (
  <Icon>
    <path d="M10 4v12M4 10h12" />
  </Icon>
);
