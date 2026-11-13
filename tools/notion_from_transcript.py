#!/usr/bin/env python3
"""X-01 C1: save a Notion MCP fetch response verbatim, without anyone retyping it.

Claude Code records every tool result in the session transcript (JSONL). This script finds the
`mcp__notion__notion-fetch` calls for one page, takes the recorded `tool_result` text exactly as
stored, writes it to a file (mode 0600) and a metadata file: transcript line, tool_use_id, record
time, byte length, SHA-256, `page_last_edited_at`, and the completeness markers the connector
reports (`truncated`, `unknown_block_count`, closing tags). It reads no credentials and calls no
network. Output belongs in a private directory, never in the public repository.

usage: notion_from_transcript.py TRANSCRIPT.jsonl PAGE_ID OUT_DIR [--all]
"""
import hashlib
import json
import os
import sys


def pairs(transcript, page_id):
    uses, results = {}, {}
    with open(transcript, 'rb') as f:
        for n, raw in enumerate(f, 1):
            try:
                o = json.loads(raw)
            except ValueError:
                continue
            content = (o.get('message') or {}).get('content')
            if not isinstance(content, list):
                continue
            for b in content:
                if b.get('type') == 'tool_use' and b.get('name') == 'mcp__notion__notion-fetch' \
                        and page_id in json.dumps(b.get('input', {})).replace('-', '').lower():
                    uses[b['id']] = n
                if b.get('type') == 'tool_result' and b.get('tool_use_id') in uses:
                    results[b['tool_use_id']] = (n, b.get('content'), o.get('timestamp'))
    return sorted(((tid,) + v for tid, v in results.items()), key=lambda x: x[1])


def main():
    if len(sys.argv) < 4:
        sys.exit(__doc__)
    transcript, page_arg, out_dir = sys.argv[1:4]
    # accept a dashed UUID or a URL as well as the 32-hex form
    hexes = ''.join(c for c in page_arg.split('?')[0].split('#')[0] if c in '0123456789abcdefABCDEF')
    page_id = hexes[-32:].lower()
    if len(page_id) != 32:
        sys.exit('PAGE_ID must contain a 32-hex Notion page id')
    found = pairs(transcript, page_id)
    if not found:
        sys.exit('no notion-fetch result for this page in the transcript')
    os.makedirs(out_dir, mode=0o700, exist_ok=True)
    for tid, line, content, ts in (found if '--all' in sys.argv else found[-1:]):
        text = content if isinstance(content, str) else ''.join(
            b.get('text', '') for b in content or [] if b.get('type') == 'text')
        raw = text.encode()
        v = json.loads(text)
        stamp = (ts or 'unknown').replace(':', '').replace('-', '')[:15]
        out = os.path.join(out_dir, f'{page_id}-mcp-fetch-{stamp}.json')
        with open(out, 'wb') as f:
            f.write(raw)
        os.chmod(out, 0o600)
        meta = {'transcript_line': line, 'tool_use_id': tid, 'recorded_at': ts, 'bytes': len(raw),
                'sha256': hashlib.sha256(raw).hexdigest(), 'page_last_edited_at': v.get('page_last_edited_at'),
                'url': v.get('url'), 'truncated': v.get('truncated'),
                'unknown_block_count': v.get('unknown_block_count'),
                'unknown_block_ids': v.get('unknown_block_ids'),
                'has_content_close': '</content>' in v.get('text', ''),
                'has_page_close': '</page>' in v.get('text', '')}
        with open(out + '.meta.json', 'w') as f:
            json.dump(meta, f, ensure_ascii=False, indent=1)
        os.chmod(out + '.meta.json', 0o600)
        print(out, meta['sha256'], meta['bytes'])


if __name__ == '__main__':
    main()
