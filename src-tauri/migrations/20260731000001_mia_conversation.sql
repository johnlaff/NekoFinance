-- A conversa que persiste entre sessões, e o que sobrevive ao gesto de apagá-la.
--
-- Três stores com durabilidades diferentes de propósito: o que a pessoa lê, o rastro técnico que
-- caduca sozinho e a proveniência de um lançamento aprovado, que é histórico financeiro e não
-- histórico de conversa. A cascata do SQLite é quem garante isso — apagar a conversa é um DELETE
-- só, e nenhum caminho de aplicação pode esquecer de limpar uma das tabelas.

-- Uma conversa por instalação. O transcript nativo do provedor mora aqui inteiro porque é ele que
-- reidrata o histórico da próxima rodada: reconstruí-lo das mensagens visíveis perderia as
-- chamadas de ferramenta e os envelopes, e o modelo receberia uma conversa que não aconteceu.
CREATE TABLE IF NOT EXISTS mia_conversation (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL,
    runtime_transcript_json TEXT NOT NULL DEFAULT '[]'
);

-- O par pergunta/resposta como a tela o desenha. `answer_json` é opaco para o backend: quem reduz
-- os eventos da rodada à resposta visível é a interface, e guardar o tipo dela aqui criaria uma
-- segunda definição do mesmo formato para divergir.
CREATE TABLE IF NOT EXISTS mia_message (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id INTEGER NOT NULL REFERENCES mia_conversation(id) ON DELETE CASCADE,
    seq INTEGER NOT NULL,
    author TEXT NOT NULL CHECK (author IN ('voce', 'mia')),
    question TEXT,
    answer_json TEXT,
    at_iso TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_mia_message_conversation_seq
    ON mia_message (conversation_id, seq);

-- O rastro técnico de uma rodada: chamadas, ferramentas, tokens, custo, provedor, modelo, erros,
-- tentativas e o motivo da parada. Nunca contém credencial — todo texto do outro lado passa pelo
-- redator antes de existir aqui.
CREATE TABLE IF NOT EXISTS mia_round_trace (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id INTEGER NOT NULL REFERENCES mia_conversation(id) ON DELETE CASCADE,
    round_id TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);

-- `created_at` é indexado porque a purga varre por idade, não por conversa.
CREATE INDEX IF NOT EXISTS idx_mia_round_trace_created_at ON mia_round_trace (created_at);
CREATE INDEX IF NOT EXISTS idx_mia_round_trace_conversation
    ON mia_round_trace (conversation_id);

-- A proveniência de uma proposta e do lançamento que ela virou. `ON DELETE SET NULL` é a decisão:
-- apagar a conversa não pode apagar de onde veio um lançamento aprovado, mas o vínculo com uma
-- conversa que não existe mais também não pode ficar apontando para o vazio.
CREATE TABLE IF NOT EXISTS mia_proposal_ledger (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id INTEGER REFERENCES mia_conversation(id) ON DELETE SET NULL,
    proposal_json TEXT NOT NULL,
    proposal_hash TEXT NOT NULL,
    decision TEXT,
    transaction_id TEXT,
    created_at TEXT NOT NULL
);
