import React from "react";

// MiaAvatar — Mia's brand mark rendered as an inline SVG.
// Self-contained; inline-style pattern (no injected stylesheet needed for an SVG).
// The cat-ear silhouette with jade fill on a dark surface-elevated background.

export function MiaAvatar({ width = 40, height = 40, className = "", style = {} }) {
  return (
    <svg
      viewBox="0 0 40 40"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      role="img"
      aria-label="Mia, copiloto financeiro"
      width={width}
      height={height}
      className={className}
      style={style}
      focusable="false"
    >
      <rect width="40" height="40" rx="12" fill="var(--surface-elevated, #1F2827)" />
      <path
        fillRule="evenodd"
        clipRule="evenodd"
        fill="var(--primary, #3FBF8F)"
        d="M11 15 L9 5 L17.5 11.5 C19 11 21 11 22.5 11.5 L31 5 L29 15 C31.5 17.5 32.5 20.3 32.5 23 C32.5 29 27.5 33.5 20 33.5 C12.5 33.5 7.5 29 7.5 23 C7.5 20.3 8.5 17.5 11 15 Z M16.6 22 C16.6 23.3 15.9 24.3 15 24.3 C14.1 24.3 13.4 23.3 13.4 22 C13.4 20.7 14.1 19.7 15 19.7 C15.9 19.7 16.6 20.7 16.6 22 Z M26.6 22 C26.6 23.3 25.9 24.3 25 24.3 C24.1 24.3 23.4 23.3 23.4 22 C23.4 20.7 24.1 19.7 25 19.7 C25.9 19.7 26.6 20.7 26.6 22 Z M20 26 L18.4 24.7 C18.9 24.3 21.1 24.3 21.6 24.7 Z"
      />
    </svg>
  );
}
