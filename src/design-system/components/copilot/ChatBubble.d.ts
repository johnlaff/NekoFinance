import * as React from "react";

export interface ChatBubbleProps {
  /** Who is speaking. mia = avatar + name + surface bubble; user = right-aligned jade-tint. */
  from?: "mia" | "user";
  /** Override the speaker name (default "Mia"). */
  name?: string;
  /** Show the animated "Mia is reading" thinking dots instead of content. */
  thinking?: boolean;
  /** Initials shown in the user avatar. */
  userInitials?: string;
  children?: React.ReactNode;
  className?: string;
}

/**
 * A chat message from Mia (the copilot) or the user. Mia messages carry the
 * blinking slit-pupil presence cue. Use the `.nk-chat__money` class on amounts
 * inside content to keep them in tabular mono.
 */
export function ChatBubble(props: ChatBubbleProps): JSX.Element;
