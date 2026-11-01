#!/usr/bin/env python3
"""What a session log shows: the agent, id and project of the session, and every tool call in
order. The reference bench reads the earlier sessions with it, and traces how the new ones
worked. It began as the prototype of `tokprof handoff --note`, which now builds the note.

Usage: sessionlog.py LOG
LOG is a Claude Code transcript or a Codex rollout, plain or .jsonl.zst.
"""
import json
import os
import re
import subprocess
import sys
from collections import Counter

HOME = os.path.expanduser('~')


def lines(path):
    """The lines of a log, decompressed on the fly when it ends in .zst."""
    if not path.endswith('.zst'):
        yield from open(path, encoding='utf-8', errors='replace')
        return
    with subprocess.Popen(['zstd', '-dc', path], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL) as zstd:
        for line in zstd.stdout:
            yield line.decode('utf-8', 'replace')
        zstd.kill()


def clip(text, n):
    text = ' '.join(text.split())
    return text if len(text) <= n else text[: n - 1] + '…'


def shown(path, cwd):
    if cwd and path.startswith(cwd.rstrip('/') + '/'):
        return path[len(cwd.rstrip('/')) + 1:]
    return '~' + path[len(HOME):] if path.startswith(HOME) else path


class Session:
    def __init__(self, path):
        self.path, self.agent, self.id, self.cwd, self.branch = path, None, None, None, None
        self.first = self.last = None
        self.prompts, self.texts, self.changed, self.commands = [], [], [], []
        self.read, self.todos = Counter(), None
        # Every tool call in order, as (tool, what it worked on).
        self.calls = []

    def seen(self, ts):
        if ts:
            self.first = self.first or ts
            self.last = ts


def claude(s, v):
    if v.get('type') in ('user', 'assistant'):
        s.seen(v.get('timestamp'))
    s.id = s.id or v.get('sessionId')
    s.cwd = s.cwd or v.get('cwd')
    s.branch = v.get('gitBranch') or s.branch
    if v.get('isSidechain'):
        return
    content = (v.get('message') or {}).get('content')
    if v.get('type') == 'user' and not v.get('isMeta') and not v.get('isCompactSummary'):
        if isinstance(content, list):
            content = '\n'.join(b.get('text', '') for b in content if isinstance(b, dict) and b.get('type') == 'text')
        text = (content or '').strip()
        command = re.search(r'<command-name>(.*?)</command-name>', text)
        if command:
            args = re.search(r'<command-args>(.*?)</command-args>', text, re.S)
            text = command.group(1) + (' ' + args.group(1).strip() if args and args.group(1).strip() else '')
        elif text.startswith('<'):
            return
        if text:
            s.prompts.append((v.get('timestamp'), text))
    elif v.get('type') == 'assistant' and isinstance(content, list):
        for b in content:
            if not isinstance(b, dict):
                continue
            if b.get('type') == 'text' and b.get('text', '').strip():
                s.texts.append((v.get('timestamp'), b['text'].strip()))
            elif b.get('type') == 'tool_use':
                name, args = b.get('name'), b.get('input') or {}
                file = args.get('file_path') or args.get('notebook_path')
                what = file or args.get('command') or args.get('pattern') or args.get('description') or ''
                s.calls.append((name, clip(str(what), 160)))
                if name in ('Edit', 'Write', 'MultiEdit', 'NotebookEdit') and file:
                    s.changed.append(file)
                elif name == 'Read' and file:
                    s.read[file] += 1
                elif name == 'Bash' and args.get('command'):
                    s.commands.append(args['command'])
                elif name == 'TodoWrite':
                    s.todos = [(t.get('status'), t.get('content', '')) for t in args.get('todos', [])]


def codex(s, v):
    p = v.get('payload') or {}
    if v.get('type') == 'session_meta':
        s.id, s.cwd = s.id or p.get('id'), s.cwd or p.get('cwd')
        # Codex records the branch when the session starts.
        branch = (p.get('git') or {}).get('branch')
        s.branch = s.branch or (branch and branch + ' at the start')
        return
    if v.get('type') != 'response_item':
        return
    s.seen(v.get('timestamp'))
    kind = p.get('type')
    if kind == 'message':
        text = '\n'.join(c.get('text', '') for c in p.get('content') or [] if isinstance(c, dict)).strip()
        if p.get('role') == 'user' and text and not text.startswith(('<', '# AGENTS.md')):
            s.prompts.append((v.get('timestamp'), text))
        elif p.get('role') == 'assistant' and text:
            s.texts.append((v.get('timestamp'), text))
    elif kind in ('function_call', 'custom_tool_call'):
        name, raw = p.get('name'), p.get('arguments') or p.get('input') or ''
        try:
            args = json.loads(raw) if isinstance(raw, str) and raw.lstrip().startswith('{') else {}
        except ValueError:
            args = {}
        code = raw if isinstance(raw, str) else ''
        if name in ('exec_command', 'shell'):
            cmd = args.get('cmd') or args.get('command')
            if isinstance(cmd, list):
                cmd = cmd[-1] if len(cmd) == 3 and str(cmd[1]).startswith('-') else ' '.join(map(str, cmd))
            if cmd:
                s.commands.append(cmd)
            s.calls.append((name, clip(str(cmd or ''), 160)))
        elif name == 'update_plan':
            s.todos = [(t.get('status'), t.get('step', '')) for t in args.get('plan', [])]
            s.calls.append((name, ''))
        else:
            s.calls.append((name, clip(code, 160)))
            # apply_patch, or the `exec` tool's JavaScript that calls the real tools.
            for m in re.finditer(r'\*\*\* (?:Update|Add|Delete) File: ([^\s"\\]+)', args.get('input', '') or code):
                s.changed.append(m.group(1))
            for m in re.finditer(r'cmd:\s*"((?:[^"\\]|\\.)*)"', code):
                s.commands.append(m.group(1).encode().decode('unicode_escape', 'replace'))


def outputs(path):
    """The text of every tool result in a session log."""
    for line in lines(path):
        try:
            v = json.loads(line)
        except ValueError:
            continue
        content = (v.get('message') or {}).get('content')
        if v.get('type') == 'user' and isinstance(content, list):
            for b in content:
                if isinstance(b, dict) and b.get('type') == 'tool_result':
                    yield json.dumps(b.get('content'), ensure_ascii=False)
        p = v.get('payload') or {}
        if v.get('type') == 'response_item' and p.get('type') in ('function_call_output', 'custom_tool_call_output'):
            yield json.dumps(p.get('output'), ensure_ascii=False)


def leaks(path, marks):
    """How often each mark turns up in the tool results of a session."""
    counts = {m: 0 for m in marks}
    for text in outputs(path):
        for m in marks:
            counts[m] += text.count(m)
    return {m: n for m, n in counts.items() if n}


def meta(path):
    """The agent, id and project of a session, from the first lines of its log."""
    s = Session(path)
    for n, line in enumerate(lines(path)):
        try:
            v = json.loads(line)
        except ValueError:
            continue
        if s.agent is None:
            s.agent = 'codex' if 'payload' in v else 'claude'
        p = v.get('payload') or {}
        s.id = s.id or (p.get('id') if v.get('type') == 'session_meta' else v.get('sessionId'))
        s.cwd = s.cwd or p.get('cwd') or v.get('cwd')
        if (s.id and s.cwd) or n > 5000:
            break
    return s


def read(path):
    s = Session(path)
    for line in lines(path):
        try:
            v = json.loads(line)
        except ValueError:
            continue
        if s.agent is None:
            s.agent = 'codex' if 'payload' in v else 'claude'
        (codex if s.agent == 'codex' else claude)(s, v)
    return s


if __name__ == '__main__':
    s = read(sys.argv[1])
    print(f'{s.agent} {s.id} {s.cwd}')
    for tool, what in s.calls:
        print(f'{tool}  {what}')
