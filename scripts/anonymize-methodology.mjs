#!/usr/bin/env node

/**
 * Anonymize private source material into a public-safe methodology pack.
 *
 * Input:  ANONYMIZE_SOURCE_DIR (required - path to private scrape package)
 * Output: METHODOLOGY_PACK_DIR (default: .methodology-pack)
 *
 * The script:
 *   1. Reads transcriptions, courses, and lesson metadata from the private source.
 *   2. Strips names, domains, URLs, handles, emails, signed links, and internal IDs.
 *   3. Extracts financial methodology content using keyword and pattern heuristics.
 *   4. Groups content into topic-based rules with neutral IDs.
 *   5. Writes the methodology pack: rules JSON, chunks JSONL, manifest, and index.
 */

import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  writeFileSync,
} from "node:fs";
import { basename, join, resolve } from "node:path";
import { createInterface } from "node:readline";

// -- config ---------------------------------------------------------------
const SOURCE_DIR = process.env.ANONYMIZE_SOURCE_DIR;

if (!SOURCE_DIR) {
  console.error(
    "ANONYMIZE_SOURCE_DIR is required. Set it to the private source directory.",
  );
  process.exit(1);
}

const PACK_DIR =
  process.env.METHODOLOGY_PACK_DIR ??
  resolve(import.meta.dirname, "../.methodology-pack");

const MIN_PARAGRAPH_CHARS = 100;
const MIN_SEGMENT_SCORE = 0.3;

// -- keyword heuristics ---------------------------------------------------
const FINANCE_KEYWORDS = new Set([
  "reserva",
  "emergência",
  "emergencia",
  "gasto",
  "gastos",
  "receita",
  "receitas",
  "orçamento",
  "orcamento",
  "dívida",
  "divida",
  "dívidas",
  "dividas",
  "crédito",
  "credito",
  "débito",
  "debito",
  "cartão",
  "cartao",
  "fatura",
  "inadimplência",
  "inadimplencia",
  "investimento",
  "investir",
  "poupança",
  "poupanca",
  "poupar",
  "riqueza",
  "patrimônio",
  "patrimonio",
  "planilha",
  "método",
  "metodo",
  "percentual",
  "porcentagem",
  "categoria",
  "classificar",
  "organizar",
  "separar",
  "controle",
  "gestão",
  "gestao",
  "finanças",
  "financas",
  "financeiro",
  "financeira",
  "autônomo",
  "autonomo",
  "autônoma",
  "autonoma",
  "lucro",
  "prejuízo",
  "prejuizo",
  "salário",
  "salario",
  "renda",
  "proventos",
  "despesa",
  "fixo",
  "variável",
  "variavel",
  "essencial",
  "supérfluo",
  "superfluo",
  "consumo",
  "proteção",
  "protecao",
  "seguro",
  "imposto",
  "planejamento",
  "estratégia",
  "estrategia",
  "regra",
  "princípio",
  "principio",
  "passo",
  "etapa",
  "ciclo",
  "meta",
  "objetivo",
  "pro-labore",
  "pró-labore",
  "pro labore",
  "distribuição",
  "distribuicao",
  "alocação",
  "alocacao",
]);

const METHODOLOGY_PATTERNS = [
  /\b(você|voce)\s+(deve|precisa|pode|deveria|tem que|vai)\b/i,
  /\b(é|e)\s+(importante|essencial|fundamental|necessário|necessario|preciso)\b/i,
  /\b(recomendo|sugiro|aconselho|indico|oriento)\b/i,
  /\b(regra|princípio|principio|fundamento|pilar)\s+(número|numero|\d|do|da)\b/i,
  /\b(passo|etapa|fase)\s+(\d|número|numero|um|dois|três|tres)\b/i,
  /\b(primeiro|segundo|terceiro|quarto|quinto)\s+(passo|etapa|regra|ponto)\b/i,
  /\b(nunca|jamais|sempre|evite|evitar)\b.+(faça|faca|gaste|compre|use|invista)\b/i,
  /\b(isso significa que|o que isso quer dizer|em outras palavras|na prática|na pratica)\b/i,
  /\b(se você|se voce)\s+(ganha|recebe|tem|possui|paga|deve|investe)\b/i,
  /\b(calcule|calcular|some|somar|divida|dividir|multiplique|multiplique)\b/i,
  /\b(exemplo|ex\.|ex:|por exemplo)\b.+(reserva|gasto|orçamento|orcamento|renda)\b/i,
];

const BANTER_MARKERS = [
  /\b(risos|rs|kkk|haha|hehe|lol)\b/i,
  /\b(bom dia|boa tarde|boa noite|oi gente|oi pessoal|e aí|e ai)\b/i,
  /\b(tudo bem|como vocês estão|como voces estao|beleza)\b/i,
  /\b(compartilha|comenta|curte|se inscreve|deixa o like)\b/i,
  /\b(vamos começar|vamos lá|vamos la|bora|partiu)\b/i,
  /\b(me digam|me contem|me fala|me conta)\b/i,
  /\b(beleza\?|ok\?|certo\?|tudo bem\?)\b/i,
  /\b(manda aí|manda ai|me manda|pergunta)\b.+(chat|comunidade|grupo)\b/i,
];

// -- identifiers to strip ------------------------------------------------
const STRIP_PATTERNS = [
  // URLs (http, https, www)
  /https?:\/\/[^\s<>"{}|\\^`[\]]+/gi,
  // Signed URLs (common CDN signing patterns)
  /https?:\/\/[^\s<>"{}|\\^`[\]]+?(?:Expires|Signature|X-Amz|token|auth)[^\s<>"{}|\\^`[\]]*/gi,
  // Email addresses
  /[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}/g,
  // Generic URLs (www.)
  /www\.[^\s<>"{}|\\^`[\]]+/gi,
  // Social media handles (@username)
  /@[a-zA-Z0-9_]{3,30}\b/g,
  // Markdown links with URLs [text](url)
  /\[([^\]]*)\]\(https?:\/\/[^\s<>"{}|\\^`[\]]+\)/gi,
  // Phone numbers (Brazilian format)
  /\(?\d{2,3}\)?\s?\d{4,5}-?\d{4}/g,
  // Numeric IDs that look like platform IDs (sequences 6+ digits standalone)
  /\b\d{6,}\b/g,
  // Hashtags
  /#[a-zA-Z0-9_á-úÁ-Ú]{2,50}\b/g,
];

// -- helpers --------------------------------------------------------------
function* walkJsonl(filePath) {
  if (!existsSync(filePath)) return;
  const stream = readFileSync(filePath, "utf-8");
  for (const line of stream.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    try {
      yield JSON.parse(trimmed);
    } catch {
      // skip malformed
    }
  }
}

function collectTranscriptionDirs(sourceDir) {
  const result = [];
  const base = join(sourceDir, "01 - Aulas, Cursos e Videos");
  if (!existsSync(base)) return result;

  function walk(dir, depth) {
    if (depth > 4) return;
    const transPath = join(dir, "transcricoes", "large-v3-turbo");
    if (existsSync(join(transPath, "segments.json"))) {
      result.push({ path: transPath, relative: dir.slice(base.length + 1) });
      return;
    }
    let entries;
    try {
      entries = readdirSync(dir, { withFileTypes: true });
    } catch {
      return;
    }
    for (const entry of entries) {
      if (entry.isDirectory()) {
        walk(join(dir, entry.name), depth + 1);
      }
    }
  }

  walk(base, 0);
  return result;
}

function stripIdentifiers(text, forbiddenTerms = []) {
  let cleaned = text;
  for (const pattern of STRIP_PATTERNS) {
    cleaned = cleaned.replace(pattern, "[removido]");
  }
  for (const term of forbiddenTerms) {
    const escaped = term.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    cleaned = cleaned.replace(new RegExp(escaped, "gi"), "[removido]");
  }
  cleaned = cleaned.replace(/\s{2,}/g, " ");
  return cleaned.trim();
}

function scoreSegment(text) {
  const lower = text.toLowerCase();
  let score = 0;
  let keywordHits = 0;

  for (const kw of FINANCE_KEYWORDS) {
    if (lower.includes(kw)) keywordHits++;
  }
  score += Math.min(keywordHits / 3, 1) * 0.4;

  let patternHits = 0;
  for (const pat of METHODOLOGY_PATTERNS) {
    if (pat.test(lower)) patternHits++;
  }
  score += Math.min(patternHits / 2, 1) * 0.4;

  let banterHits = 0;
  for (const pat of BANTER_MARKERS) {
    if (pat.test(lower)) banterHits++;
  }
  score -= banterHits * 0.1;

  const hasStructured = /^\s*[-•*\d]+[.)]\s/.test(text) || text.includes(":");
  if (hasStructured) score += 0.1;

  return Math.max(0, Math.min(1, score));
}

function classifyTopic(text) {
  const lower = text.toLowerCase();
  const topics = [];

  if (/(reserva|emergência|emergencia|fundo)/.test(lower)) topics.push("reserva");
  if (/(dívida|divida|inadimpl|endivida)/.test(lower)) topics.push("dividas");
  if (/(crédito|credito|cartão|cartao|fatura|limite)/.test(lower))
    topics.push("credito");
  if (/(débito|debito|conta corrente|conta-corrente)/.test(lower))
    topics.push("debito");
  if (
    /(investimento|investir|poupança|poupanca|renda fixa|renda variável|renda variavel)/.test(
      lower,
    )
  )
    topics.push("investimentos");
  if (/(orçamento|orcamento|planejamento|planilha|categoria|classificar)/.test(lower))
    topics.push("orcamento");
  if (
    /(autônomo|autonomo|pj|pessoa jurídica|pessoa juridica|pro-labore|pró-labore)/.test(
      lower,
    )
  )
    topics.push("autonomos");
  if (
    /(gasto|despesa|consumo|supérfluo|superfluo|essencial|fixo|variável|variavel)/.test(
      lower,
    )
  )
    topics.push("gastos");
  if (/(renda|salário|salario|receita|provento|lucro)/.test(lower))
    topics.push("renda");
  if (/(seguro|proteção|protecao|imposto|previdência|previdencia)/.test(lower))
    topics.push("protecao");

  return topics.length > 0 ? topics : ["geral"];
}

function neutralId(topic, index) {
  const short = topic.substring(0, 4);
  return `method.${short}.${String(index).padStart(3, "0")}`;
}

function chunkText(text, maxChars = 800) {
  const sentences = text.split(/(?<=[.!?])\s+/);
  const chunks = [];
  let current = "";

  for (const sentence of sentences) {
    if ((current + " " + sentence).length > maxChars && current.length > 0) {
      chunks.push(current.trim());
      current = sentence;
    } else {
      current += (current ? " " : "") + sentence;
    }
  }
  if (current.trim().length > 0) chunks.push(current.trim());
  return chunks;
}

// -- main pipeline --------------------------------------------------------
function main() {
  console.log("=== Neko Finance: Anonymization Pipeline ===\n");
  console.log("Source:", SOURCE_DIR);
  console.log("Output:", PACK_DIR);

  if (!existsSync(SOURCE_DIR)) {
    console.error("Source directory not found:", SOURCE_DIR);
    process.exit(1);
  }

  // Prepare output
  const rulesDir = join(PACK_DIR, "rules");
  const chunksDir = join(PACK_DIR, "chunks");
  const indexesDir = join(PACK_DIR, "indexes");
  for (const d of [PACK_DIR, rulesDir, chunksDir, indexesDir]) {
    if (!existsSync(d)) mkdirSync(d, { recursive: true });
  }

  // Collect transcriptions
  const transDirs = collectTranscriptionDirs(SOURCE_DIR);
  console.log(`Found ${transDirs.length} transcriptions\n`);

  // Load forbidden terms for stripping
  const forbiddenFile = resolve(PACK_DIR, "../.private-forbidden-patterns");
  const forbiddenTerms = [];
  if (existsSync(forbiddenFile)) {
    const lines = readFileSync(forbiddenFile, "utf-8").split("\n");
    for (const line of lines) {
      const trimmed = line.trim();
      if (!trimmed || trimmed.startsWith("#")) continue;
      if (trimmed.length < 3) continue;
      forbiddenTerms.push(trimmed);
    }
  }
  console.log(`Forbidden terms loaded: ${forbiddenTerms.length}\n`);

  // Process
  const allChunks = [];
  const ruleGroups = new Map();
  let totalSegments = 0;
  let keptSegments = 0;
  let ruleCounter = 0;

  for (const { path: transPath } of transDirs) {
    const segmentsFile = join(transPath, "segments.json");
    if (!existsSync(segmentsFile)) continue;

    const rawSegments = JSON.parse(readFileSync(segmentsFile, "utf-8"));
    if (!Array.isArray(rawSegments)) continue;

    let contextWindow = "";

    for (const seg of rawSegments) {
      totalSegments++;
      const rawText = seg.text || "";
      if (rawText.length < 30) continue;

      contextWindow = (contextWindow + " " + rawText).slice(-1200);

      const score = scoreSegment(contextWindow);
      if (score < MIN_SEGMENT_SCORE) continue;

      keptSegments++;
      const cleaned = stripIdentifiers(rawText, forbiddenTerms);
      if (cleaned.length < MIN_PARAGRAPH_CHARS) continue;

      const topics = classifyTopic(cleaned);

      for (const topic of topics) {
        ruleCounter++;
        const ruleId = neutralId(topic, ruleCounter);
        const hash = createHash("sha256").update(cleaned).digest("hex").slice(0, 8);
        const chunkId = `${ruleId}-${hash}`;

        const chunk = {
          id: chunkId,
          ruleId,
          topic,
          text: cleaned,
          score: Math.round(score * 100) / 100,
          charCount: cleaned.length,
        };

        allChunks.push(chunk);

        if (!ruleGroups.has(topic)) {
          ruleGroups.set(topic, []);
        }
        ruleGroups.get(topic).push(chunk);
      }

      contextWindow = "";
    }
  }

  // Build rules from groups
  const rules = [];
  for (const [topic, chunks] of ruleGroups) {
    // Take top-scoring chunks as rule content, merge similar ones
    const sorted = chunks.sort((a, b) => b.score - a.score);
    const top = sorted.slice(0, Math.min(20, sorted.length));

    // Generate a summary from the top chunks
    const combined = top.map((c) => c.text).join("\n");
    const sources = top.map((c) => c.id);

    rules.push({
      id: `method.${topic}.overview`,
      topic,
      title: `Metodologia: ${topic.charAt(0).toUpperCase() + topic.slice(1)}`,
      description: `Regras anonimizadas sobre ${topic} extraídas da metodologia financeira privada.`,
      chunkCount: chunks.length,
      topChunks: sources.slice(0, 5),
      sourcePolicy: "anonymized-private-derived",
    });
  }

  // Write chunks
  const chunksFile = join(chunksDir, "methodology-chunks.jsonl");
  const chunksOut = allChunks.map((c) => JSON.stringify(c)).join("\n") + "\n";
  writeFileSync(chunksFile, chunksOut);
  console.log(
    `Chunks:      ${allChunks.length} written to chunks/methodology-chunks.jsonl`,
  );

  // Write rules
  const rulesFile = join(rulesDir, "methodology-rules.json");
  writeFileSync(rulesFile, JSON.stringify(rules, null, 2));
  console.log(
    `Rules:       ${rules.length} topics written to rules/methodology-rules.json`,
  );

  // Write CSV index for FTS
  const indexFile = join(indexesDir, "methodology-index.csv");
  const header = "id,topic,ruleId,score,charCount\n";
  const rows = allChunks
    .map((c) => `${c.id},${c.topic},${c.ruleId},${c.score},${c.charCount}`)
    .join("\n");
  writeFileSync(indexFile, header + rows);
  console.log(`FTS index:   CSV written to indexes/methodology-index.csv`);

  // Write manifest
  const manifest = {
    schemaVersion: 1,
    name: "private-finance-methodology",
    sourcePolicy: "anonymized-private-derived",
    generatedAt: new Date().toISOString(),
    stats: {
      transcriptionsProcessed: transDirs.length,
      totalSegments,
      keptSegments,
      anonymizedChunks: allChunks.length,
      topics: rules.length,
    },
    rules: rules.map((r) => r.id),
    topics: [...ruleGroups.keys()],
  };

  writeFileSync(join(PACK_DIR, "manifest.json"), JSON.stringify(manifest, null, 2));
  console.log(`\nManifest:    ${PACK_DIR}/manifest.json`);

  // Privacy self-check using already-loaded forbidden terms
  let leaks = 0;
  for (const chunk of allChunks) {
    for (const term of forbiddenTerms) {
      if (chunk.text.toLowerCase().includes(term.toLowerCase())) {
        console.warn(`WARN: possible leak of "${term}" in chunk ${chunk.id}`);
        leaks++;
      }
    }
  }

  console.log(
    `\nPrivacy check: ${leaks > 0 ? `${leaks} potential leaks found` : "clean"}`,
  );
  console.log("=== Pipeline complete ===");
}

main();
