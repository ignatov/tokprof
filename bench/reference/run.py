#!/usr/bin/env python3
"""Reference bench: how Claude Code and Codex carry on from an earlier session when a new
session gets only a reference to it, and what that costs against resuming.

For each earlier session, three new headless sessions get the same read-only task: find out
what we did there, what is done and what is left. Each gets a different reference:
- id: only the session id;
- path: the line `tokprof handoff` prints, with the id and the path of the log;
- note: what `tokprof handoff --note` prints, the line and a note built from the log.

The new sessions run in the earlier session's project, with the agent's default model or the
one given after the log. Claude Code may only read and run shell commands (no Edit or Write, at
most $5). Codex runs in its read-only sandbox. tokprof then measures each new session from its
own log, and records its forecast of resuming the earlier session.

Usage: run.py OUT_DIR [--variants id,path,note] [--with claude|codex] LOG[@MODEL] [LOG[@MODEL]...]
Each LOG is the log of an earlier session: a Claude Code transcript or a Codex rollout. With
@MODEL, its new sessions use that model, for example the earlier session's own. --variants runs
only some of the variants, for example to repeat one that failed. --with starts the new
sessions in that agent instead of the earlier session's own, so one agent picks up the other's
session.
"""
import concurrent.futures
import glob
import json
import os
import re
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sessionlog  # noqa: E402

TASK = (
    'Разберись, чем мы в ней занимались, что уже сделано и что осталось сделать, и ответь коротко, '
    'не длиннее 15 строк. Ничего не меняй: ни файлы, ни git, ни задачи в трекере.'
)
VARIANTS = ('id', 'path', 'note')
# Words of the path and note prompts, in both languages the bench used: an id-only run whose tool
# results contain them has read another run's prompt.
LEAKS = ('Its log:', 'A note about it follows', 'её лог:', 'Заметка о ней')
HOME = os.path.expanduser('~')


def tokprof(*args):
    return subprocess.run(['tokprof', *args], capture_output=True, text=True, check=True,
                          env={**os.environ, 'NO_COLOR': '1'}).stdout


def prompt(variant, old):
    if variant == 'id':
        return f'Continue from the earlier session {old.id}. {TASK}'
    if variant == 'path':
        return f'{tokprof("handoff", old.path).strip()} {TASK}'
    return f'{tokprof("handoff", "--note", old.path).rstrip()}\n\n{TASK}'


def run(agent, cwd, model, text, out):
    """Runs one headless session; returns its id, wall time and the CLI's own cost."""
    started = time.time()
    if agent == 'claude':
        cmd = ['claude', '-p', text, '--output-format', 'json', '--permission-mode', 'dontAsk',
               '--allowedTools', 'Read', 'Grep', 'Glob', 'Bash', 'Agent', 'Task', 'TodoWrite', 'ToolSearch',
               '--disallowedTools', 'Edit', 'Write', 'NotebookEdit', '--max-budget-usd', '5']
        cmd += ['--model', model] if model else []
    else:
        cmd = ['codex', 'exec', '--sandbox', 'read-only', '--skip-git-repo-check', '-C', cwd, '--json',
               '-o', out + '.answer.md']
        cmd += ['-m', model] if model else []
        cmd.append(text)
    with open(out + '.stdout', 'w') as stdout, open(out + '.stderr', 'w') as stderr:
        subprocess.run(cmd, cwd=cwd, stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr, timeout=1800)
    seconds = time.time() - started
    if agent == 'claude':
        result = json.load(open(out + '.stdout'))
        with open(out + '.answer.md', 'w') as f:
            f.write(result.get('result') or '')
        return result.get('session_id'), seconds, result.get('total_cost_usd')
    thread = None
    for line in open(out + '.stdout'):
        event = json.loads(line)
        if event.get('type') == 'thread.started':
            thread = event['thread_id']
    return thread, seconds, None


def new_log(agent, session):
    pattern = (f'{HOME}/.claude/projects/*/{session}.jsonl' if agent == 'claude'
               else f'{HOME}/.codex/sessions/**/rollout-*-{session}.jsonl*')
    found = glob.glob(pattern, recursive=True)
    return found[0] if found else None


def measure(session, out):
    """The X-ray of a new session; returns its spend, requests and final context."""
    xray = tokprof('scan', session)
    with open(out + '.xray.txt', 'w') as f:
        f.write(xray)
    get = lambda pattern: (re.search(pattern, xray) or [None, None])[1]
    return {
        'spend': get(r'spend\s+(\$[\d,.]+)'),
        'requests': get(r'· (\d+) requests'),
        'subagents': get(r'(\d+) subagents') or '0',
        'context': get(r'context\s+\S+\s+[\d.]+%\s+([\d.]+[kM]?)') or get(r'context\s+([\d.]+[kM]?) tokens'),
    }


def resume_forecast(old):
    """What tokprof forecasts for resuming the earlier session: its next request, cold."""
    xray = tokprof('scan', old.path)
    forecast = re.search(r'next\s+≈ (\$[\d,.]+) for one request, re-writing ([\d.]+[kM]?)', xray)
    span = re.search(r'\(([\d.]+[smhd])\) · (\d+) requests', xray)
    return {'agent': old.agent, 'variant': 'resume', 'forecast': True, 'session': old.id,
            'spend': forecast and forecast[1], 'context': forecast and forecast[2],
            'span': span and span[1], 'requests': span and span[2], 'log_mb': round(os.path.getsize(old.path) / 1e6, 1)}


def main():
    out_dir, args = sys.argv[1], sys.argv[2:]
    def option(flag):
        if flag not in args:
            return None
        i = args.index(flag)
        value = args[i + 1]
        del args[i:i + 2]
        return value

    variants = tuple((option('--variants') or ','.join(VARIANTS)).split(','))
    agent_with = option('--with')
    os.makedirs(out_dir, exist_ok=True)
    jobs, rows = [], []
    for arg in args:
        path, _, model = arg.partition('@')
        old = sessionlog.meta(path)
        name = f'{old.agent}-{(old.id or "")[:8]}'
        rows.append({**resume_forecast(old), 'name': name})
        agent = agent_with or old.agent
        for variant in variants:
            # Files of a run by the other agent end in -by-<agent>.
            out = os.path.join(out_dir, f'{name}-{variant}' + (f'-by-{agent}' if agent != old.agent else ''))
            jobs.append((old, name, agent, model or None, variant, out))
    # The id variants run first and alone: the prompts of the other variants name the log, and
    # an agent that searches its logs for the id would find them.
    for batch in ([j for j in jobs if j[4] == 'id'], [j for j in jobs if j[4] != 'id']):
        if batch:
            rows += run_batch(batch)
    order = ('resume',) + VARIANTS
    rows.sort(key=lambda r: (r['name'], order.index(r['variant'])))
    with open(os.path.join(out_dir, 'results.json'), 'w') as f:
        json.dump(rows, f, ensure_ascii=False, indent=1)


def run_batch(jobs):
    """Runs the sessions of a batch in parallel; returns their rows. A prompt is written only when
    its batch starts, so no earlier batch can come across it."""
    rows = []
    with concurrent.futures.ThreadPoolExecutor(len(jobs)) as pool:
        futures = {}
        for old, name, agent, model, variant, out in jobs:
            text = prompt(variant, old)
            with open(out + '.prompt.md', 'w') as f:
                f.write(text + '\n')
            futures[pool.submit(run, agent, old.cwd, model, text, out)] = (old, name, agent, model, variant, out)
        for future in concurrent.futures.as_completed(futures):
            old, name, agent, model, variant, out = futures[future]
            try:
                session, seconds, cli_usd = future.result()
            except Exception as e:  # noqa: BLE001 - one failed run must not stop the others
                print(f'{name} {variant} by {agent}: failed: {e}', file=sys.stderr)
                continue
            row = {'agent': old.agent, 'name': name, 'by': agent, 'variant': variant, 'model': model, 'session': session,
                   'seconds': round(seconds), 'cli_usd': cli_usd, **measure(session, out)}
            log = new_log(agent, session)
            if log:
                calls = sessionlog.read(log).calls
                row['calls'] = len(calls)
                found = sessionlog.leaks(log, LEAKS) if variant == 'id' else {}
                if found:
                    row['contaminated'] = 'its tool results hold the prompt of a path or note run: ' + ', '.join(f'{m!r} ×{n}' for m, n in found.items())
                with open(out + '.trace.txt', 'w') as f:
                    f.write('\n'.join(f'{tool}  {what}' for tool, what in calls) + '\n')
            rows.append(row)
            print(json.dumps(row, ensure_ascii=False), flush=True)
    return rows


if __name__ == '__main__':
    main()
