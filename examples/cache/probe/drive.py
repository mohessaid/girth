import json, subprocess, sys, time, random, string, os
arm, wait_s, ping_ms, out = sys.argv[1], int(sys.argv[2]), sys.argv[3], sys.argv[4]
random.seed(arm + str(time.time()))
big = 'Session tag ' + arm + str(time.time()) + '. Here is the source of a Rust CLI for reference; just acknowledge it.\n' + open(sys.argv[7]).read()[:150000]
env = dict(os.environ, PROBE_PING_MS=ping_ms, PROBE_LOG=out + '.ping.jsonl')
p = subprocess.Popen(['claude', '-p', '--input-format', 'stream-json', '--output-format', 'stream-json', '--verbose',
    '--model', 'claude-haiku-5-5', '--plugin-dir', sys.argv[5], '--max-budget-usd', '1'],
    stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=open(out + '.err', 'w'), text=True, env=env, cwd=sys.argv[6])
log = open(out, 'w')
def send(text):
    p.stdin.write(json.dumps({'type': 'user', 'message': {'role': 'user', 'content': text}}) + '\n'); p.stdin.flush()
def until_result():
    for line in p.stdout:
        log.write(line); log.flush()
        try: e = json.loads(line)
        except: continue
        if e.get('type') == 'result': return e
send(big + '\n\nReply with just: ok')
until_result()
time.sleep(wait_s)
send('Reply with just: done')
until_result()
p.stdin.close(); p.wait(timeout=120)
