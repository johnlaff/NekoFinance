import * as React from "react";

/**
 * A chat message from Mia (the copilot) or the user. Mia messages show the avatar,
 * a blinking slit-pupil presence cue, and a surface-tinted bubble; user messages
 * right-align in a jade-tint bubble. Use `.nk-chat__money` on amounts inside content
 * for tabular mono figures.
 * @startingPoint section="Copilot" subtitle="ChatBubble — copilot message bubble" viewport="560x200"
 */
export interface ChatBubbleProps {
  /** Who is speaking. "mia" = left-aligned avatar + name + surface bubble; "user" = right-aligned jade-tint. */
  from?: "mia" | "user";
  /** Override the speaker display name shown above the bubble (default "Mia"). Only shown for from="mia". */
  name?: string;
  /** Show the animated three-dot "Mia is thinking" indicator instead of children. */
  thinking?: boolean;
  /** One or two initials shown in the user's avatar circle (default "You"). */
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
