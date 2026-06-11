A chat message from Mia (copilot) or the user. Mia messages show the avatar + a blinking slit-pupil presence cue; user messages right-align in a jade tint.

```jsx
<ChatBubble from="user" userInitials="AT">How much did we spend on dining last month?</ChatBubble>

<ChatBubble from="mia">
  <p>You spent <span className="nk-chat__money">$486.20</span> on dining in May — about <b>11%</b> of variable spending.</p>
</ChatBubble>

<ChatBubble from="mia" thinking />
```

Use `thinking` for the loading state. Wrap amounts in `<span className="nk-chat__money">` for tabular figures. Pair with `<Citation>` / tool-result blocks inside Mia's bubble for sourced answers.
