import type { Register } from 'claude-code';
const rows: string[] = [];
// Where the rows go: change this to a folder of your own.
const OUT = '/tmp/girth-orderprobe.jsonl';
async function flush($: any) { await $.fs.write(OUT, rows.join('\n') + '\n').catch(() => {}); }
// A cheap, stable string hash (FNV-1a).
function h(s: string) { let x = 0x811c9dc5; for (let i = 0; i < s.length; i++) { x ^= s.charCodeAt(i); x = Math.imul(x, 16777619) >>> 0; } return x.toString(16); }
const seen = new Map<string, string>();
export const register: Register = (on) => {
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
      if (e.event === 'api_request') {
        const a: any = e.attributes;
        rows.push(JSON.stringify({ k: 'req', cr: a.cache_read_tokens, cw: a.cache_creation_tokens, at: Date.now() }));
        await flush($);
      }
    } catch {}
    return next(e);
  }).catch(async ($: any, e: any, next: any) => next(e));
};
