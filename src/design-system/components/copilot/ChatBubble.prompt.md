Mensagem de chat da Mia (copiloto) ou do usuário — a Mia exibe avatar + nome + cue piscante; mensagens do usuário ficam alinhadas à direita com tint jade.

```jsx
<ChatBubble from="user" userInitials="VC">
  O mês passado tem vários gastos com comida sem categoria. Pode categorizá-los?
</ChatBubble>

<ChatBubble from="mia">
  <p>
    Encontrei <b>3 gastos com comida sem categoria</b> em maio, somando{" "}
    <span className="nk-chat__money">R$ 131,70</span>. Dois são em lugares que você costuma dividir.
  </p>
</ChatBubble>

<ChatBubble from="mia" thinking />
```

`from` ("mia" | "user") determina o lado e a cor da bolha. `thinking` substitui o conteúdo pelos três pontos animados. `userInitials` define as iniciais no avatar do usuário. Envolva valores monetários em `<span className="nk-chat__money">` para manter o mono tabelado. Tokens principais: `--surface`, `--border`, `--primary-quiet`, `--font-money`, `--ease-calm`, `--ease-standard`, `--radius-md`.
