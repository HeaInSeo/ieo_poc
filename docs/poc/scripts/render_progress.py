#!/usr/bin/env python3
"""Render progress views from state.json; never infer execution or acceptance.

Run from any directory. --root selects docs/poc; --check never writes.
Reports and raw evidence are immutable inputs referenced by the progress ledger.
"""
import argparse
import hashlib
import json
from pathlib import Path
import sys
from urllib.parse import urlsplit

STATUSES = ('미착수', '준비됨', '진행', '검수 중', '검수 완료', '조건 대기', '종료 미충족')


def cell(value):
    if value is None:
        return '미확인'
    if isinstance(value, (dict, list)):
        value = json.dumps(value, ensure_ascii=False, separators=(',', ':'))
    return str(value).replace('|', '\\|').replace('\n', ' / ').replace('\r', '')


def local_ref(root, ref):
    if not isinstance(ref, str) or not ref:
        raise ValueError('비어 있거나 잘못된 증거/보고 참조')
    parsed = urlsplit(ref)
    if parsed.scheme:
        if parsed.scheme != 'https' or not parsed.netloc:
            raise ValueError('외부 증거는 https URL이어야 함')
        return None
    p = Path(ref)
    if p.is_absolute() or '..' in p.parts:
        raise ValueError('보고/증거 경로는 docs/poc 내부 상대 경로여야 함')
    resolved = (root / p).resolve()
    if not resolved.is_relative_to(root.resolve()) or not resolved.is_file():
        raise ValueError('실제 파일이 없는 보고/증거 경로: ' + ref)
    return resolved


def link(ref, label='보고'):
    if not ref:
        return '없음 · 미실행'
    href = ref if ref.startswith('https://') else '../' + ref
    return '[' + cell(label) + '](' + href.replace(' ', '%20').replace(')', '%29') + ')'


def validate_state(root, s):
    task_ids = [t['task_id'] for t in s['tasks']]
    if len(task_ids) != len(set(task_ids)):
        raise ValueError('중복 작업 ID')
    for track, key in [('main', 'main_tasks'), ('followup', 'followup_tasks')]:
        if sum(t['track'] == track for t in s['tasks']) != s['baselines'][key]:
            raise ValueError('기준선과 작업 수 불일치: ' + track)
    # Extra tasks (X-*, amendment A3) are listed apart and never enter the 73/12 counts.
    extra_ids = [t['task_id'] for t in s.get('extra_tasks', [])]
    if len(extra_ids) != len(set(extra_ids)) or set(extra_ids) & set(task_ids):
        raise ValueError('중복 추가 작업 ID')
    for t in s.get('extra_tasks', []):
        if t.get('track') != 'extra' or not t['task_id'].startswith('X-'):
            raise ValueError('추가 작업은 track=extra, ID X-*: ' + t['task_id'])
        if t['status'] not in STATUSES:
            raise ValueError('알 수 없는 추가 작업 상태: ' + t['task_id'])
        if t['status'] == '검수 완료' and (
            t.get('verification_level') != 'verified' or not t.get('evidence')
        ):
            raise ValueError('검수 완료에 verified/증거가 없음: ' + t['task_id'])
    exp_ids = [e['experiment_id'] for e in s['experiments']]
    if len(exp_ids) != len(set(exp_ids)):
        raise ValueError('중복 실험 ID')
    for track, key in [('main', 'main_experiments'), ('followup', 'followup_candidates')]:
        if sum(e['track'] == track for e in s['experiments']) != s['baselines'][key]:
            raise ValueError('기준선과 실험 수 불일치: ' + track)
    for t in s['tasks']:
        if t['status'] not in STATUSES:
            raise ValueError('알 수 없는 작업 상태: ' + t['task_id'])
        if t['status'] == '검수 완료' and (
            t.get('verification_level') != 'verified' or not t.get('evidence')
        ):
            raise ValueError('검수 완료에 verified/증거가 없음: ' + t['task_id'])
    for e in s['experiments']:
        if not set(e.get('task_ids', [])).issubset(task_ids):
            raise ValueError('실험의 미정의 작업 참조')
        runs = [(r['run_id'], r.get('revision', 1)) for r in e.get('runs', [])]
        if len(runs) != len(set(runs)):
            raise ValueError('중복 실험 회차 revision')
        if e.get('evaluation_closed') and not e.get('runs'):
            raise ValueError('실제 회차 없는 평가 종료')

    # Check explicit refs before writing any view. URLs are not fetched or claimed verified.
    def refs(v):
        if isinstance(v, dict):
            if v.get('is_template'):
                raise ValueError('양식을 실제 회차로 집계할 수 없음')
            for k, x in v.items():
                if k == 'latest_report' and x:
                    local_ref(root, x)
                elif k == 'evidence' and isinstance(x, list):
                    for entry in x:
                        ref = entry if isinstance(entry, str) else entry.get('ref') or entry.get('path') or entry.get('url')
                        if ref:
                            local_ref(root, ref)
                refs(x)
        elif isinstance(v, list):
            for x in v:
                refs(x)
    refs(s)
    for key in ('current_task', 'next_task'):
        if s.get(key) is not None and s[key] not in task_ids:
            raise ValueError('미정의 현재/다음 작업')
    events = [(e['event_id'], e.get('event_revision', 1)) for e in s.get('events', [])]
    if len(events) != len(set(events)):
        raise ValueError('중복 사건 revision')


def render(root):
    raw = (root / 'progress/state.json').read_bytes()
    s = json.loads(raw)
    validate_state(root, s)
    digest = hashlib.sha256(raw).hexdigest()
    stamp = (f"계획 {s['plan_version']} · snapshot `{s['snapshot_id']}` · "
             f"원장 SHA-256 `{digest}`\n\n"
             "자동 생성 화면 · 원본은 state.json · 손편집 금지\n\n")
    def table(headers, rows):
        return '| ' + ' | '.join(headers) + ' |\n| ' + ' | '.join(['---'] * len(headers)) + ' |\n' + ''.join(
            '| ' + ' | '.join(str(x) for x in row) + ' |\n' for row in rows) + '\n'
    status = '# PoC 진행과 최근 결과\n\n' + stamp
    status += f"**현재: {cell(s['phase'])} · 다음 작업: {cell(s['next_task'])}**\n\n"
    status += '주계획과 조건부 후속은 별도 진행률이다. 실험 평가 종료·공수·제품 완성률과 구분한다. ZIP은 전달 시점 스냅샷이다.\n\n'
    rows = []
    for track, title, key in [('main','주계획','main_tasks'),('followup','후속 실험','followup_tasks')]:
        ts = [t for t in s['tasks'] if t['track'] == track]
        done = sum(t['status'] == '검수 완료' for t in ts)
        rows.append([title,f"{done}/{s['baselines'][key]}",f"{100 * done / s['baselines'][key]:.1f}%"])
    status += table(['구간','검수 완료','단순 작업 수 진행률'],rows)
    status += table(['구간','상태별 수'],[[title, ' · '.join(f"{st} {sum(t['status']==st for t in s['tasks'] if t['track']==track)}" for st in STATUSES)] for track,title in [('main','주계획'),('followup','후속')]])
    extras = s.get('extra_tasks', [])
    if extras:
        status += '추가 작업은 사용자 지시로 더한 별도 범위다. 주계획 73·후속 12의 검수 완료 수와 진행률에 넣지 않는다.\n\n'
        status += table(['추가 작업','상태','선행','최근 보고'],[[cell(t['task_id'])+' · '+cell(t['title']),cell(t['status']),cell(', '.join(t.get('prerequisites', [])) or '없음'),link(t.get('latest_report'))] for t in extras])
    status += table(['구간','평가 종료'],[[title,f"{sum(bool(e['evaluation_closed']) for e in s['experiments'] if e['track']==track)}/{s['baselines'][key]}"] for track,title,key in [('main','주계획 실험','main_experiments'),('followup','후속 후보','followup_candidates')]])
    status += '## 지금 확인할 내용\n\n'
    status += table(['항목','기록'],[
        ['현재 작업',cell(s['current_task']) if s.get('current_task') else '없음 · 실행 전' if s['phase']=='실행 전' else '현재 배정 없음'],['다음 작업',cell(s.get('next_task'))],
        ['마지막 실제 작업 관측',cell(s['last_actual_work_observed_at']) if s.get('last_actual_work_observed_at') else '없음 · 미실행'],
        ['화면 snapshot 작성',cell(s['snapshot_created_at'])],['보고 owner',cell(s.get('reporting_owner'))],
        ['열람 상태',cell(s['visibility'])],['고정 사용자 링크',cell(s['delivery'].get('fixed_status_link'))],
        ['대화 전달',cell(s['delivery'].get('state'))]])
    status += '## 최근 결과와 대기\n\n'
    reported = sorted([t for t in s['tasks'] if t.get('latest_report')],key=lambda t:t.get('last_observed_at') or '',reverse=True)
    status += table(['작업','상태·판정','최근 보고'],[[cell(t['task_id']),cell(t['status'])+' / '+cell(t.get('result_state')),link(t['latest_report'])] for t in reported[:8]]) if reported else '실제 작업/실험 보고 없음 · 미실행\n\n'
    blocked=[t for t in s['tasks'] if t['status'] in ('조건 대기','종료 미충족')]
    status += table(['작업','사유·다음 행동'],[[cell(t['task_id']),cell(t.get('next_action') or t.get('unresolved') or '근거 미확인')] for t in blocked]) if blocked else '현재 실행의 대기/실패 기록 없음. 계획상 선행 작업은 아직 미수행.\n\n'
    status += table(['필수 기능/전환','상태','증거','관측 시각'],[[cell(k),cell(v.get('status')),cell(v.get('evidence')) if v.get('evidence') else '없음',cell(v.get('last_observed_at'))] for k,v in s['milestones'].items()])
    status += '## 미확정과 다음 행동\n\n'+''.join('- '+cell(x)+'\n' for x in s['unresolved'])+'\n'
    status += '다음 의존 작업 착수 전·제어를 돌려받는 지점마다 원장을 갱신하고 재생성한다. 응답 없는 동안 새 관측을 추정하지 않는다. 강제 종료 뒤 실제 외부 효과를 대조한다.\n\n'
    status += '- [주계획·후속 작업](TASKS.md) · [모든 실험 회차](RESULTS.md) · [중단/재개](RESUME.md)\n- [원장](state.json) · [파생 사건 기록](events.jsonl) · [인계 지시](../DEV_AGENT_HANDOFF.md)\n'
    task_md='# 주계획·후속 작업 상태\n\n'+stamp
    for track,title in [('main','주계획'),('followup','조건부 후속')]:
        task_md+='## '+title+'\n\n'
        task_md+=table(['작업','종류·구분','상태','결과·근거 수준','관측 시각','최근 보고','다음 행동'],[
            [cell(t['task_id'])+' · '+cell(t['title']),cell(t['kind'])+' / '+cell(t['group']),cell(t['status']),cell(t.get('result_state'))+' / '+cell(t.get('verification_level')),cell(t.get('last_observed_at')),link(t.get('latest_report')),cell(t.get('next_action'))]
            for t in s['tasks'] if t['track']==track])
    if s.get('extra_tasks'):
        task_md+='## 추가 작업 (73·12 완료 수 미산입)\n\n'
        task_md+=table(['작업','종류','상태','결과·근거 수준','관측 시각','최근 보고','다음 행동'],[
            [cell(t['task_id'])+' · '+cell(t['title']),cell(t['kind']),cell(t['status']),cell(t.get('result_state'))+' / '+cell(t.get('verification_level')),cell(t.get('last_observed_at')),link(t.get('latest_report')),cell(t.get('next_action'))]
            for t in s['extra_tasks']])
    task_md+='[현황](STATUS.md) · [실험 결과](RESULTS.md)\n'
    results='# 실험·후속 후보 결과'+'\n\n'+stamp
    for track,title in [('main','주계획 실험'),('followup','후속 후보')]:
        results+='## '+title+'\n\n'
        for e in s['experiments']:
            if e['track']!=track:continue
            results+='### '+cell(e['experiment_id'])+' · '+cell(e['title'])+'\n\n'
            results+=f"연결 작업: {cell(e['task_ids'])} · 실행 {cell(e['execution_state'])} · 판정 {cell(e['verdict'])} · 평가 종료 {cell(e['evaluation_closed'])} · 처분 {cell(e['adoption'])}\n\n"
            results+='최근 보고: '+link(e.get('latest_report'))+'\n\n'
            if not e.get('runs'):
                results+='회차 없음 · 미실행\n\n';continue
            for run in e['runs']:
                results+=f"#### 회차 {cell(run['run_id'])} / revision {cell(run.get('revision',1))}\n\n"
                results+=table(['필드','실제 기록'],[[cell(k),link(v) if k=='latest_report' else cell(v)] for k,v in run.items()])
    results+='[현황](STATUS.md) · [작업 상태](TASKS.md)\n'
    resume='# 중단·재개 기록\n\n'+stamp+table(['항목','기록'],[[cell(k),cell(v)] for k,v in s.get('resume',{}).items()])+'원장과 실제 외부 상태를 대조해 재개한다. [현황](STATUS.md)\n'
    events=''.join(json.dumps(e,ensure_ascii=False,separators=(',',':'))+'\n' for e in s.get('events',[]))
    return {'STATUS.md':status,'TASKS.md':task_md,'RESULTS.md':results,'RESUME.md':resume,'events.jsonl':events}


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--root',type=Path,default=Path(__file__).resolve().parents[1])
    ap.add_argument('--check',action='store_true')
    args=ap.parse_args()
    try:
        outputs=render(args.root)
        stale=[]
        progress=args.root/'progress'
        if args.check:
            for name,text in outputs.items():
                p=progress/name
                if not p.exists() or p.read_bytes()!=text.encode():stale.append(name)
            leftovers=sorted(x.name for x in progress.glob('*.tmp'))
            if leftovers:stale.append('남은 임시 파일 '+', '.join(leftovers))
        else:
            # 01-06: an interrupted earlier run may leave *.tmp files and a mix of old/new views.
            # Remove stale temporaries, write every view to a temporary first, then swap them in;
            # a rerun from state.json always reproduces the complete set (verified by test_render_progress.py).
            for x in progress.glob('*.tmp'):x.unlink()
            temporaries=[]
            for name,text in outputs.items():
                temporary=progress/(name+'.tmp');temporary.write_text(text,encoding='utf-8');temporaries.append((temporary,progress/name))
            for temporary,target in temporaries:temporary.replace(target)
        if stale:raise ValueError('재생성 필요: '+', '.join(stale))
        print(json.dumps({'ok':True,'mode':'check' if args.check else 'render','files':list(outputs)},ensure_ascii=False))
    except (ValueError,KeyError,OSError,TypeError) as e:
        print(str(e),file=sys.stderr);return 1
    return 0


if __name__=='__main__':
    sys.exit(main())
