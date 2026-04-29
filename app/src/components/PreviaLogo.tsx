interface PreviaLogoProps {
  className?: string;
}

export function PreviaLogo({ className = "h-6 w-6" }: PreviaLogoProps) {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 636 636"
      aria-label="Previa"
      role="img"
      className={className}
      focusable="false"
    >
      <g transform="translate(-31 -404)">
        <path
          d="M150 0h336a150 150 0 0 1 150 150v336a150 150 0 0 1-150 150H150A150 150 0 0 1 0 486V150A150 150 0 0 1 150 0Z"
          transform="translate(31 404)"
          fill="#3c83f6"
        />
        <path
          d="M812.412 1129.1H671.2l421.386-428.668V1129.1h-100V945.188Z"
          transform="translate(-533.196 -192.432)"
          fill="#ffffff"
        />
      </g>
    </svg>
  );
}
