import type { Register } from 'claude-code';
const rows: string[] = [];
// Where the rows go: change this to a folder of your own.
const OUT = '/tmp/girth-orderprobe.jsonl';
async function flush($: any) { await $.fs.write(OUT, rows.join('\n') + '\n').catch(() => {}); }
// A cheap, stable string hash (FNV-1a).
function h(s: string) { let x = 0x811c9dc5; for (let i = 0; i < s.length; i++) { x ^= s.charCodeAt(i); x = Math.imul(x, 16777619) >>> 0; } return x.toString(16); }
const seen = new Map<string, string>();
async function servers($: any, when: string) {
  const u: any = await $.session.usage({ breakdown: 'summary' });
  const by: Record<string, number> = {};
  for (const t of u?.context?.breakdown?.mcpTools ?? []) by[t.serverName] = (by[t.serverName] ?? 0) + 1;
  rows.push(JSON.stringify({ k: 'tools', when, by, at: Date.now() }));
  await flush($);
}
export const register: Register = (on) => {
  on('turn.start', async ($, e, next) => { try { await servers($, 'start'); } catch {} return next(e); }).catch(async ($: any, e: any, next: any) => next(e));
  on('turn.step', async function* ($: any, e: any, next: any) { try { await servers($, 'step'); } catch {} yield* next(e); }).catch(async function* ($: any, e: any, next: any) { yield* next(e); });
  on('tool.list', async ($, e, next) => {
    const r: any = await next(e);
    try {
      const list: any[] = Array.isArray(r) ? r : [];
      rows.push(JSON.stringify({ k: 'list', n: list.length, order: h(list.map((t) => t.name).join(',')),
        schemas: h(list.map((t) => t.name + JSON.stringify(t.inputSchema ?? null)).join('|')),
        descs: h(list.map((t) => t.description).join('|')), at: Date.now() }));
      await flush($);
    } catch {}
    return r;
  }).catch(async ($: any, e: any, next: any) => next(e));
  on('tool.describe', async ($, e, next) => {
    const r: any = await next(e);
    try {
      const d = h(String(r?.description ?? ''));
      const before = seen.get(e.tool);
      if (before !== undefined && before !== d) rows.push(JSON.stringify({ k: 'changed', tool: e.tool, at: Date.now() }));
      if (['Skill', 'Agent', 'ListAgents', 'Bash'].includes(e.tool)) rows.push(JSON.stringify({ k: 'desc', tool: e.tool, len: String(r?.description ?? '').length, at: Date.now() }));
      seen.set(e.tool, d);
      await flush($);
    } catch {}
    return r;
  }).catch(async ($: any, e: any, next: any) => next(e));
  on('telemetry.log', { to: 'collector' }, async ($, e, next) => {
    try {
      if (e.event === 'mcp_server_connection') {
        const a: any = e.attributes;
        // Only these fields: the record also carries account details.
        rows.push(JSON.stringify({ k: 'mcp', server: a.server_name ?? a.mcp_server_name ?? null, status: a.status ?? a.connection_status ?? null, keys: Object.keys(a).filter((k) => !/user|org|email|account|session/i.test(k)), at: Date.now() }));
        await flush($);
      }
      if (e.event === 'api_request') {
        const a: any = e.attributes;
        rows.push(JSON.stringify({ k: 'req', cr: a.cache_read_tokens, cw: a.cache_creation_tokens, at: Date.now() }));
        await flush($);
      }
    } catch {}
    return next(e);
  }).catch(async ($: any, e: any, next: any) => next(e));
};
