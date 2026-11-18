#!/usr/bin/env python3
"""T09 offline result export. The progress ledger is read-only."""

import argparse
from collections import defaultdict
from copy import deepcopy
from datetime import datetime
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile

from jsonschema import Draft202012Validator, FormatChecker, ValidationError

from render_progress import cell, local_ref, render, validate_state
from validate_contracts import require_timestamp, validate_report


FORMAT = 'console.poc.result-export.v0'
MANIFEST = 'Console_Coordinator_Sprints_v0.json'
FOLLOWUP = 'Followup_Execution_v1.3.3.json'
LEDGER = 'progress/state.json'
HISTORY = '01_plan_and_review/Console_Coordinator_PoC_Validation_Plan_v1.2.md'
RUN_TEMPLATE = 'reports/templates/result.template.json'
ADOPTION = {'채택', '부분 채택', '보류', '기각'}
WORK_KINDS = {'integration': '구현', 'implementation': '구현', 'regression': '검증', 'experiment': '평가'}
TOKEN_FIELDS = {'input', 'output', 'cache_read', 'cache_creation'}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def encode(value):
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2,
                       allow_nan=False) + '\n').encode()


def canonical_digest(value):
    return sha(json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':'),
                          allow_nan=False).encode())


def indexed(rows, key):
    result = {}
    for row in rows:
        value = row[key]
        require(isinstance(value, str) and value.strip() and value not in result,
                f'invalid or duplicate {key}: {value}')
        result[value] = row
    return result


def cli_json(binary, *args, codes=(0,)):
    result = subprocess.run([binary, *map(str, args)], capture_output=True, text=True,
                            encoding='utf-8')
    require(result.returncode in codes, result.stderr.strip() or result.stdout.strip()
            or f'{binary} exited {result.returncode}')
    return json.loads(result.stdout)


class Sources:
    def __init__(self, root):
        self.root = root
        self.content = {}

    def read(self, ref):
        if ref not in self.content:
            path = local_ref(self.root, ref)
            require(path is not None, f'offline evidence requires a local copy: {ref}')
            self.content[ref] = path.read_bytes()
        return self.content[ref]

    def json(self, ref):
        return json.loads(self.read(ref))

    def readable(self, refs):
        missing = []
        for ref in refs:
            try:
                self.read(ref)
            except (ValueError, OSError, TypeError):
                missing.append(ref)
        return missing

    def digests(self):
        return {ref: sha(raw) for ref, raw in self.content.items()}


def ledger_view(sources, manifest):
    state = sources.json(LEDGER)
    validate_state(sources.root, state)
    main = indexed((t for spec in manifest['sprints'] for t in spec['tasks']), 'task_id')
    followup = indexed(sources.json(FOLLOWUP)['tasks'], 'id')
    require(state['plan_version'] == manifest['plan_version'], 'ledger plan version mismatch')
    require(set(main).isdisjoint(followup), 'main and followup IDs overlap')
    expected = {**dict.fromkeys(main, 'main'), **dict.fromkeys(followup, 'followup')}
    require({t['task_id']: t['track'] for t in state['tasks']} == expected,
            'ledger ID/track mismatch with current manifests')
    require(len(main) == manifest['baseline_tasks'] and
            len(followup) == sources.json(FOLLOWUP)['baseline_tasks'], 'manifest denominator mismatch')
    state_tasks = indexed(state['tasks'], 'task_id')
    progress = {}
    for track in ('main', 'followup'):
        ids = sorted(i for i in expected if expected[i] == track)
        progress[track] = {
            'accepted': sum(state_tasks[i]['status'] == '검수 완료' for i in ids),
            'total': len(ids),
        }
    sources.read(HISTORY)
    progress['historical'] = {'version': 'v1.2', 'accepted': 0, 'total': 76, 'source': HISTORY}
    progress['snapshot_id'] = state['snapshot_id']
    progress['ledger_sha256'] = sha(sources.read(LEDGER))
    tasks = [{**t, 'legacy_v1_2_id': (main[t['task_id']]['legacy_v1_2_id']
              if t['track'] == 'main' else followup[t['task_id']]['legacy'])}
             for t in state['tasks']]
    return main, state_tasks, progress, tasks, state.get('extra_tasks', [])


def review_for(sources, authority, run, ref, as_of):
    review_ref = run.get('operational_report_ref')
    if not review_ref or sources.readable([review_ref]):
        return None
    pinned = authority.get(review_ref)
    if not pinned:
        return None
    try:
        raw = sources.read(review_ref)
        if sha(raw) != pinned:
            return None
        review = json.loads(raw)
        validate_report(sources.root, review)
        finished = run['finished_at']
        require_timestamp(finished)
        observed = datetime.fromisoformat(review['observed_at'].upper())
        if (observed <= as_of and (finished is None or datetime.fromisoformat(finished.upper()) <= observed)
                and review['report_type'] == 'coordinator_verdict'
                and review['evidence_level'] == 'verified'
                and review['actor']['role'] == 'coordinator'
                and review['actor']['name'] == run['verification']['reviewer']
                and review['actor']['name'] != run['actor']['name']
                and review['task_id'] == run['task_id']
                and 'sha256:' + sha(sources.read(ref)) in review['evidence']):
            refs = [e for e in review['evidence'] if not re.fullmatch(r'sha256:[a-f0-9]{64}', e)]
            if refs and not sources.readable(refs):
                return review
    except (KeyError, TypeError, ValueError, OSError, ValidationError):
        return None
    return None


def classify_run(sources, authority, mode, goal, entry, run, current, as_of):
    ref = entry['result']
    issues = []
    evidence = run['evidence_refs'] + run['verification']['evidence']
    if not run['evidence_refs'] or not run['verification']['evidence']:
        issues.append('missing_original_or_review_evidence')
    missing = sources.readable(evidence)
    if missing:
        issues.append('unreadable_evidence')
    if run['execution_kind'] != mode:
        issues.append('execution_kind_mismatch')
    target = goal['target']
    if (not all(target.get(k) for k in ('spec_revision', 'repo_sha', 'profile_revision'))
            or run['inputs'] != target):
        issues.append('target_version_mismatch')
    review = review_for(sources, authority, run, ref, as_of)
    if run['verification']['level'] != 'verified' or review is None:
        issues.append('independent_result_review_missing')
    if not run['expected'] or run['observed'] is None:
        issues.append('expectation_or_observation_missing')
    if run['verdict'] == '미판정' or (run['verdict'] in {'PASS', 'FAIL'} and not run['finished_at']):
        issues.append('evaluation_pending')
    observed = run['observed'] if isinstance(run['observed'], dict) else {}
    covered = observed.get('covered_scopes', [])
    require(isinstance(covered, list) and all(isinstance(s, str) for s in covered),
            f'invalid covered scopes: {ref}')
    if not set(covered) <= set(goal['scopes']):
        issues.append('observation_outside_goal')
    if not current:
        issues.append('historical_result')
    verdict = run['verdict'] if not issues else 'INCONCLUSIVE'
    if verdict == 'N/A' and goal['required']:
        issues.append('required_scope_na')
        verdict = 'INCONCLUSIVE'
    verified_evaluation = verdict in {'PASS', 'FAIL'}

    decision = None
    decision_ref = entry.get('disposition')
    effective = '미판정'
    if decision_ref and not sources.readable([decision_ref]):
        decision = sources.json(decision_ref)
        scopes = decision['scopes']
        require(decision['decision'] in ADOPTION and isinstance(scopes, list)
                and all(isinstance(s, str) for s in scopes), f'invalid disposition: {decision_ref}')
        approved = bool(review and current and run['execution_kind'] == mode
                        and decision['result_sha256'] == sha(sources.read(ref))
                        and 'sha256:' + sha(sources.read(decision_ref)) in review['evidence']
                        and decision['proposed_by'] != review['actor']['name']
                        and decision.get('reason'))
        choice = decision['decision']
        if not approved:
            issues.append('disposition_approval_missing')
        elif choice in {'채택', '부분 채택'}:
            if (verdict == 'PASS' and scopes and set(scopes) <= set(covered)
                    and (set(scopes) == set(goal['scopes'])) == (choice == '채택')):
                effective = choice
            else:
                issues.append('adoption_scope_unproven')
        elif choice == '보류':
            if all(decision.get(k) for k in ('owner', 'resume_condition', 'recheck_at')):
                require_timestamp(decision['recheck_at'])
                effective = choice
            else:
                issues.append('hold_resume_details_missing')
        elif verdict in {'PASS', 'FAIL'}:
            effective = choice
        else:
            issues.append('rejection_basis_unproven')
    elif decision_ref:
        issues.append('disposition_unreadable')
    elif goal['kind'] == '평가' or verdict in {'FAIL', 'INCONCLUSIVE'}:
        issues.append('disposition_missing')

    return {
        'goal_id': entry['goal_id'], 'candidate': entry['candidate'], 'result': ref,
        'plan_version': run['plan_version'], 'task_id': run['task_id'], 'experiment_id': run['experiment_id'],
        'run_id': run['run_id'], 'revision': run['revision'], 'current': current,
        'inputs': run['inputs'], 'reported_verdict': run['verdict'], 'verdict': verdict,
        'expected': run['expected'], 'observed': run['observed'],
        'original_evidence': evidence, 'missing_evidence': missing,
        'review': run.get('operational_report_ref'), 'disposition_ref': decision_ref,
        'reported_adoption': run['adoption'], 'disposition': decision, 'adoption': effective,
        'adopted_scopes': sorted(set(decision['scopes'])) if effective in {'채택', '부분 채택'} else [],
        'verified_scopes': sorted(set(covered)) if verdict == 'PASS' else [],
        'validated_assets': observed.get('validated_assets', []) if verified_evaluation else [],
        'records': sorted(entry['records']),
        'issues': issues, 'reported_cost': run['cost'],
    }


def results(sources, authority, bundle, goals, as_of):
    required_fields = set(sources.json(RUN_TEMPLATE))
    chains = defaultdict(dict)
    for entry in bundle['runs']:
        goal = goals[entry['goal_id']]
        ref = entry['result']
        run = sources.json(ref)
        revision = run['revision']
        require(required_fields <= run.keys() and run['schema_version'] == 'console.poc.run.v1'
                and run['is_template'] is False and isinstance(run['run_id'], str) and run['run_id']
                and type(revision) is int and revision > 0
                and isinstance(entry['candidate'], str) and entry['candidate']
                and run['verdict'] in {'PASS', 'FAIL', 'INCONCLUSIVE', 'N/A', '미판정'},
                f'invalid result report: {ref}')
        require(run['plan_version'] != bundle['plan_version'] or run['task_id'] == goal['source_task_id'],
                f'result task does not match goal: {ref}')
        key = run['plan_version'], run['task_id'], run['experiment_id'] or '', run['run_id']
        require(revision not in chains[key], f'duplicate run revision: {key}/{revision}')
        chains[key][revision] = entry, run

    rows, costs = [], defaultdict(dict)
    for key, chain in sorted(chains.items()):
        latest = max(chain)
        first_entry, first_run = chain[min(chain)]
        records = set()
        for rev, (entry, run) in sorted(chain.items()):
            require((entry['goal_id'], entry['candidate']) ==
                    (first_entry['goal_id'], first_entry['candidate']), f'run identity changed: {key}')
            require(run['execution_kind'] == first_run['execution_kind'],
                    f'run execution kind changed: {key}/{rev}')
            previous = chain[rev - 1][0]['result'] if rev - 1 in chain else None
            require((rev == 1 or previous is not None) and run['previous_run_ref'] == previous,
                    f'broken result revision chain: {key}/{rev}')
            require(isinstance(entry['records'], list) and len(entry['records']) == len(set(entry['records'])),
                    f'duplicate evidence record: {entry["result"]}')
            records.update(entry['records'])
            current = rev == latest and run['plan_version'] == bundle['plan_version']
            row = classify_run(sources, authority, bundle['mode'], goals[entry['goal_id']], entry,
                               run, current, as_of)
            rows.append(row)
            if rev == latest and run['execution_kind'] == bundle['mode']:
                costs[entry['goal_id']][key] = sorted(records)
    return rows, costs


def budgets(sources, goals, costs, as_of, collector, evidence_schema, temp):
    used = set()
    output = {}
    for goal_id, goal in sorted(goals.items()):
        policy = goal['budget']
        runs = costs.get(goal_id, {})
        missing = []
        files = []
        for key, refs in sorted(runs.items()):
            if not refs:
                missing.append('/'.join(key))
            for ref in refs:
                if sources.readable([ref]):
                    missing.append(ref)
                    continue
                record = sources.json(ref)
                evidence_schema.validate(record)
                require(record['model_request'] == (record['usage']['applicability'] == 'model'),
                        f'inconsistent model usage applicability: {ref}')
                identity = (record['task_id'], record['attempt_id'], record['command_id'])
                require(identity not in used, f'duplicate evidence record: {identity}')
                require(record['task_id'] == key[1], f'evidence record task mismatch: {ref}')
                used.add(identity)
                path = temp / str(len(used)) / 'record.json'
                path.parent.mkdir()
                path.write_bytes(sources.read(ref))
                files.append(path)
        summary = cli_json(collector, 'summarize', *files) if files else None
        require(summary is None or summary['records'] == len(files), 'collector record count mismatch')
        usage = summary['usage'] if summary else {}
        measured = usage.get('tokens_measured_sum')
        fields = policy.get('token_fields', [])
        require(isinstance(fields, list) and len(set(fields)) == len(fields)
                and set(fields) <= TOKEN_FIELDS, f'invalid token definition: {goal_id}')
        tokens_measured = sum(measured[f] for f in fields) if fields and measured else None
        tokens_total = None
        if not missing and runs and fields and not usage.get('attribution_mismatches'):
            if usage.get('completeness') == 'complete':
                tokens_total = tokens_measured
            elif usage.get('completeness') == 'no_model_runs':
                tokens_total = 0
        wall_measured = summary['wall_ms_sum'] if summary else None
        wall_ms = wall_measured if not missing else None
        limits = {k: policy.get(k) for k in ('attempts', 'wall_ms', 'tokens')}
        require(all(v is None or type(v) is int and v >= 0 for v in limits.values()),
                f'invalid budget ceiling: {goal_id}')
        values = {'attempts': len(runs), 'wall_ms': wall_ms, 'tokens': tokens_total}
        reasons = [f'{k}_limit_reached' for k, amount in values.items()
                   if limits[k] is not None and
                   (amount if amount is not None else {'wall_ms': wall_measured,
                    'tokens': tokens_measured}.get(k) or 0) >= limits[k]]
        deadline = policy.get('deadline')
        require_timestamp(deadline)
        if deadline and as_of >= datetime.fromisoformat(deadline.upper()):
            reasons.append('deadline_reached')
        if missing or runs and (tokens_total is None or wall_ms is None):
            reasons.append('usage_incomplete')
        if None in limits.values() or not deadline or not fields:
            reasons.append('budget_unbound')
        output[goal_id] = {
            'policy': policy, 'attempts': len(runs), 'wall_ms': wall_ms,
            'wall_ms_measured': wall_measured, 'tokens_measured': tokens_measured,
            'tokens_total': tokens_total, 'missing_records': missing, 'summary': summary,
            'automatic_retry': not reasons, 'stop_reasons': reasons,
        }
    return output


def assets_for(sources, rows, declarations, work):
    by_result = {r['result']: r for r in rows}
    assets = {}
    for asset_id, item in sorted(declarations.items()):
        provenance = ('ref', 'version', 'sha256', 'origin', 'usage_terms', 'contract',
                      'dependencies', 'scope', 'target', 'remaining', 'validation_result')
        missing = [key for key in provenance if key not in item or item[key] is None or item[key] == '']
        reasons = ['missing_' + key for key in missing]
        content_match = not missing and not sources.readable([item['ref']]) and (
            sha(sources.read(item['ref'])) == item['sha256'])
        if not missing and not content_match:
            reasons.append('asset_content_unverified')
        row = by_result.get(item.get('validation_result'))
        match = {k: item.get(k) for k in ('asset_id', 'sha256', 'version', 'contract', 'scope')}
        independent = content_match and row is not None and match in row['validated_assets']
        tasks = sorted(w['task_id'] for w in work.values() if asset_id in w['asset_ids'])
        state = '보존 전용'
        if content_match:
            state = ('편입 대기' if tasks else '검증됨') if independent else '후보'
            if not independent:
                reasons.append('asset_scope_unverified')
        assets[asset_id] = {**item, 'status': state, 'issues': reasons, 'task_ids': tasks}
    return assets


def work_and_goals(goals, work, rows, assets, usage, state_tasks, repo_id):
    for w in work.values():
        require(w['goal_id'] in goals and w['purpose'] in WORK_KINDS, f'invalid work: {w["task_id"]}')
        require(w['scope'] in goals[w['goal_id']]['scopes'] and w['role'] and w['rollback'] and w['output'],
                f'missing work scope, role, output or rollback: {w["task_id"]}')
        require(len(w['after']) == len(set(w['after'])), f'duplicate prerequisite: {w["task_id"]}')
    goal_rows = defaultdict(list)
    for row in rows:
        if row['current']:
            goal_rows[row['goal_id']].append(row)
    withheld = {goal_id: {r['candidate'] for r in goal_rows[goal_id] if r['adoption'] in {'보류', '기각'}}
                for goal_id in goals}
    covered, standalone, verified = {}, {}, {}
    for goal_id, goal in goals.items():
        eligible = [r for r in goal_rows[goal_id] if r['candidate'] not in withheld[goal_id]]
        covered[goal_id] = {scope for row in eligible for scope in row['adopted_scopes']}
        standalone[goal_id] = [r for r in eligible if goal['kind'] == '검증' and r['disposition_ref'] is None]
        verified[goal_id] = {scope for row in standalone[goal_id] for scope in row['verified_scopes']}
    items = {}
    for task_id, w in sorted(work.items()):
        goal_id, scope = w['goal_id'], w['scope']
        candidates = [r for r in goal_rows[goal_id] if r['candidate'] == w.get('candidate')]
        adopted = (w.get('candidate') not in withheld[goal_id]
                   and any(scope in r['adopted_scopes'] for r in candidates))
        reasons = []
        if w['purpose'] == 'experiment':
            followup = w.get('followup') or {}
            if not all(followup.get(k) for k in ('hypothesis', 'change', 'control', 'pass_condition',
                                                'stop_condition', 'resume_condition')):
                reasons.append('followup_conditions_missing')
            if candidates and all(r['adoption'] == '기각' for r in candidates):
                reasons.append('candidate_rejected')
            if candidates and all(r['verdict'] == 'INCONCLUSIVE' for r in candidates):
                reasons.append('resume_evidence_pending')
            reasons.extend(usage[goal_id]['stop_reasons'])
        elif w['purpose'] == 'integration' and not adopted:
            reasons.append('adoption_unproven')
        elif w['purpose'] == 'regression' and not adopted:
            if not any(r['candidate'] == w.get('candidate') and scope in r['verified_scopes']
                       for r in standalone[goal_id]):
                reasons.append('regression_evidence_unproven')
        elif w['purpose'] == 'implementation':
            has_basis = adopted if w.get('candidate') else scope in covered[goal_id]
            if not has_basis:
                reasons.append('required_scope_unproven')
        for asset_id in w['asset_ids']:
            asset = assets.get(asset_id, {})
            if asset.get('status') not in {'편입 대기', '검증됨'} or asset.get('target') != repo_id:
                reasons.append('asset_unproven:' + asset_id)
        for dependency in sorted(w['after']):
            if state_tasks.get(dependency, {}).get('status') != '검수 완료':
                reasons.append('prerequisite_pending:' + dependency)
        items[task_id] = {**w, 'after': sorted(w['after']), 'state': '조건 대기' if reasons else
                          ('편입 대기' if w['purpose'] == 'integration' else '검토 대기'),
                          'issues': reasons, 'results': [r['result'] for r in candidates]}
    mapping = {}
    for goal_id, goal in sorted(goals.items()):
        related = [w for w in items.values() if w['goal_id'] == goal_id]
        unwritten = set(goal['scopes']) - {w['scope'] for w in related}
        mapping[goal_id] = {
            **goal, 'covered_scopes': sorted(covered[goal_id]),
            'verified_scopes': sorted(verified[goal_id]),
            'unproven_scopes': sorted(set(goal['scopes']) - covered[goal_id] - verified[goal_id]),
            'missing_work_scopes': sorted(unwritten) if goal['required'] else [],
            'tasks': [w['task_id'] for w in related], 'results': [r['result'] for r in goal_rows[goal_id]],
        }
    return items, mapping


def make_spec(source, bundle, goals, items, catalog, revision, input_digest):
    spec = deepcopy(source)
    details = bundle['draft']
    spec.update(sprint_id=details['sprint_id'], title=details['title'], revision=revision,
                registration_state='draft', tasks=[])
    spec['schedule'].update(planned_start_at=None, deadline_at=None, deadline_state='undecided',
                            entry_condition='대상 범위의 독립 검토와 발행 승인',
                            unresolved_reason='대상 환경의 실측과 승인 일정이 필요하다.')
    spec['unresolved_bindings'] = [*spec['unresolved_bindings'],
                                  '독립 검토와 발행 승인', '대상 범위 실제 결과 연결(09-05)']
    spec['sources'].append({'kind': 'result_export', 'ref': 'input.json', 'revision': str(revision),
                            'digest': input_digest, 'observed_at': bundle['as_of']})
    for w in items.values():
        goal = goals[w['goal_id']]
        base = catalog[goal['source_task_id']]
        task_id = w['task_id']
        estimate = w.get('estimate') or {'state': 'unestimated', 'range_seconds': None, 'evidence': [],
                                         'reason': '남은 작업의 비교 가능한 실측 범위가 미확정이다.'}
        spec['tasks'].append({
            'task_id': task_id, 'kind': WORK_KINDS[w['purpose']],
            'requirement_ids': base['requirement_ids'], 'acceptance': base['acceptance'],
            'prerequisites': [{'task_id': dep, 'plan_version': bundle['plan_version'],
                               'required_state': 'accepted'} for dep in w['after']],
            'outputs': [{'artifact_id': task_id + '-output', 'description': w['output'], 'required': True}],
            'verification': base['verification'], 'exit_condition': base['exit_condition'],
            'references': ['T09', 'P33', goal['source_task_id'], *w['results']],
            'legacy_v1_2_id': None, 'status': '조건 대기' if w['issues'] else '미착수', 'evidence': [],
            'estimate': estimate,
        })
    spec.pop('digest')
    spec['digest'] = canonical_digest(spec)
    return spec


def diff(previous, current):
    changes = {}
    for key in ('goals', 'work', 'assets', 'budgets'):
        before, after = previous[key], current[key]
        changes[key] = sorted(i for i in before.keys() | after.keys() if before.get(i) != after.get(i))
    changes['source_refs'] = sorted(
        k for k in previous['source_digests'].keys() | current['source_digests'].keys()
        if previous['source_digests'].get(k) != current['source_digests'].get(k))
    changed_refs = set(changes['source_refs'])
    impacted = {r['goal_id'] for r in current['runs'] if changed_refs.intersection(
        [r['result'], r['review'], r['disposition_ref'], *r['original_evidence'], *r['records']])}
    changed = set(changes['work'])
    for task_id, w in current['work'].items():
        if (w['goal_id'] in impacted or w['goal_id'] in changes['goals'] or w['goal_id'] in changes['budgets']
                or set(w['asset_ids']).intersection(changes['assets'])):
            changed.add(task_id)
    while True:
        dependents = {i for i, w in current['work'].items() if changed.intersection(w['after'])}
        if dependents <= changed:
            break
        changed.update(dependents)
    changes['reverify_work'] = sorted(changed.intersection(current['work']))
    return changes


def render_draft(result, spec):
    def table(headers, rows):
        return '\n'.join('| ' + ' | '.join(map(cell, r)) + ' |'
                         for r in (headers, ['---'] * len(headers), *rows)) + '\n\n'

    text = f"# {spec['title']}\n\n"
    text += f"`{spec['sprint_id']}` revision {spec['revision']} · {result['mode']} · 검토용 초안\n\n"
    text += '[등록 명세](sprint.json) · [결과·처분·자산·변경 근거](export.json) · [진척표](progress/STATUS.md)\n\n'
    text += '독립 검토와 발행 승인 대기. 제품 완료 미확인. 실제 결과 연결은 09-05에서 검증한다.\n\n'
    text += '## 목표와 수용 기준\n\n'
    text += table(['목표', '요구 ID', '범위', '입증 대기', '작업 누락', '작업', '수용 기준'],
                  [[goal_id, ', '.join(g['requirement_ids']), '필수' if g['required'] else '선택',
                    ', '.join(g['unproven_scopes']), ', '.join(g['missing_work_scopes']),
                    ', '.join(g['tasks']), g['acceptance']['criteria']]
                   for goal_id, g in result['goals'].items()])
    text += '## 결과와 도입 결정\n\n'
    text += table(['목표', '후보', '회차', '버전', '판정', '처분', '채택 범위', '검토 사유'],
                  [[r['goal_id'], r['candidate'], f"{r['run_id']} r{r['revision']}",
                    '현재' if r['current'] else '이력', r['verdict'], r['adoption'],
                    ', '.join(r['adopted_scopes']), ', '.join(r['issues'])]
                   for r in result['runs']])
    text += '## 재사용 자산\n\n'
    text += table(['자산', '버전', '상태', '대상', '관련 작업', '남은 보완'],
                  [[asset_id, a.get('version'), a['status'], a.get('target'),
                    ', '.join(a['task_ids']), a.get('remaining')]
                   for asset_id, a in result['assets'].items()])
    text += '## 남은 작업\n\n'
    text += table(['작업', '목표·범위', '유형', '담당', '상태', '선행', '자산', '복귀 조건'],
                  [[i, f"{w['goal_id']} / {w['scope']}", w['purpose'], w['role'], w['state'],
                    ', '.join(w['after']), ', '.join(w['asset_ids']), w['rollback']]
                   for i, w in result['work'].items()])
    text += '작업별 공수 근거는 등록 명세에 기록했다. 스프린트 기한은 미확정이다.\n\n'
    text += '## 누적 사용량\n\n'
    text += table(['목표', '회차', '계측 시간(ms)', '계측 토큰 합', '전체 토큰', '반복 중단 사유'],
                  [[i, b['attempts'], b['wall_ms_measured'], b['tokens_measured'], b['tokens_total'],
                    ', '.join(b['stop_reasons'])] for i, b in result['budgets'].items()])
    return text


def export(args, temp, previous):
    sources = Sources(args.root)
    raw_input = args.input.read_bytes()
    raw_authority = args.authority.read_bytes() if args.authority else b'{}'
    bundle, authority_doc = json.loads(raw_input), json.loads(raw_authority)
    mode = bundle['mode']
    require(bundle['format'] == 'console.poc.export-input.v0' and mode in {'actual', 'synthetic'},
            'unsupported export input')
    authority = authority_doc.get('approvals', {}) if authority_doc.get('mode') == mode else {}
    manifest = sources.json(MANIFEST)
    catalog, state_tasks, progress, tasks, extra = ledger_view(sources, manifest)
    require(bundle['plan_version'] == manifest['plan_version'], 'export plan version mismatch')
    require_timestamp(bundle['as_of'])
    as_of = datetime.fromisoformat(bundle['as_of'].upper())
    seeds = indexed(manifest['sprints'], 'sprint_id')
    draft = bundle['draft']
    require(draft['sprint_id'] not in seeds, 'draft collides with registered sprint')
    seed = seeds[draft['seed_sprint_id']]
    goals = indexed(bundle['goals'], 'goal_id')
    require(goals, 'at least one goal is required')
    indexed(goals.values(), 'source_task_id')
    for goal in goals.values():
        base = catalog[goal['source_task_id']]
        require(type(goal['required']) is bool and isinstance(goal['scopes'], list) and goal['scopes']
                and all(isinstance(s, str) and s for s in goal['scopes'])
                and len(set(goal['scopes'])) == len(goal['scopes']), 'invalid goal scope')
        goal.update(kind=base['kind'], requirement_ids=base['requirement_ids'],
                    acceptance=base['acceptance'])
    work = indexed(bundle['work'], 'task_id')
    declarations = indexed(bundle['assets'], 'asset_id')
    rows, costs = results(sources, authority, bundle, goals, as_of)
    evidence_schema = Draft202012Validator(
        json.loads((args.root / '../../contracts/evidence-record-v0.schema.json').read_bytes()),
        format_checker=FormatChecker())
    usage = budgets(sources, goals, costs, as_of, args.collector, evidence_schema, temp)
    assets = assets_for(sources, rows, declarations, work)
    items, mapping = work_and_goals(goals, work, rows, assets, usage, state_tasks,
                                    seed['target']['repo_id'])
    for w in items.values():
        if w.get('estimate'):
            require(not sources.readable(w['estimate']['evidence']),
                    f'unreadable estimate evidence: {w["task_id"]}')
    source_digests = sources.digests()
    work_ids = dict(previous['work_ids']) if previous else {}
    for i, item in items.items():
        identity = [goals[item['goal_id']]['source_task_id'], item['goal_id'], item['purpose'], item['scope']]
        require(work_ids.setdefault(i, identity) == identity,
                f'work ID reused for a different objective: {i}')
    archived = {}
    if previous:
        by_ref = {r['result']: r for r in rows}
        for i, old in previous['goals'].items():
            if old['required']:
                require(i in goals and goals[i]['required'] and
                        goals[i]['source_task_id'] == old['source_task_id'] and
                        set(old['scopes']) <= set(goals[i]['scopes']),
                        f'previous required scope omitted or changed: {i}')
        for old in previous['runs']:
            ref = old['result']
            require(ref in by_ref and by_ref[ref]['goal_id'] == old['goal_id']
                    and set(old['records']) <= set(by_ref[ref]['records']),
                    f'previous run or accounting evidence omitted: {ref}')
            for evidence_ref in (ref, old['disposition_ref'], old['review'],
                                 *old['original_evidence'], *old['records']):
                saved = previous['source_digests'].get(evidence_ref)
                if saved:
                    require(saved == source_digests.get(evidence_ref),
                            f'previous result or evidence changed in place: {evidence_ref}')
        archived = previous['budget_policies']
    budget_policies = {**archived, **{i: g['budget'] for i, g in goals.items()}}
    for i in archived.keys() & goals.keys():
        require(archived[i] == budget_policies[i], f'budget policy changed within a result series: {i}')
    normalized = json.loads(raw_input)
    for name, key in (('goals', 'goal_id'), ('runs', 'result'), ('assets', 'asset_id'), ('work', 'task_id')):
        normalized[name].sort(key=lambda v: v[key])
    normalized = encode(normalized)
    converter_sha = sha(Path(__file__).read_bytes())
    fingerprint = canonical_digest([FORMAT, converter_sha, sha(normalized), sha(raw_authority), source_digests])
    if previous and previous['input_digest'] == fingerprint:
        return None, None, None, None
    revision = 1 if previous is None else previous['revision'] + 1
    spec = make_spec(seed, bundle, goals, items, catalog, revision, sha(normalized))
    path = temp / 'sprint.json'
    path.write_bytes(encode(spec))
    registration = cli_json(args.spec, 'check', path, '--manifest', args.root / MANIFEST, codes=(0, 1))
    require(not registration['outcome']['rejections'],
            json.dumps(registration['outcome']['rejections'], ensure_ascii=False))
    result = {
        'format': FORMAT, 'input_sha256': sha(raw_input), 'authority_sha256': sha(raw_authority),
        'input_digest': fingerprint, 'converter_sha256': converter_sha, 'source_digests': source_digests,
        'mode': mode, 'plan_version': bundle['plan_version'],
        'sprint_id': spec['sprint_id'], 'revision': revision, 'sprint_digest': spec['digest'],
        'approval': {'generation': 'draft', 'independent_review': 'pending', 'publication': 'pending',
                     'execution': 'not_started'},
        'product_completion': 'unverified', 'actual_linkage': 'pending_09-05',
        'progress': progress, 'poc_tasks': tasks, 'extra_tasks': extra,
        'runs': rows, 'budgets': usage, 'assets': assets,
        'work': items, 'work_ids': work_ids, 'goals': mapping, 'budget_policies': budget_policies,
        'required_goals_unresolved': sorted(i for i, g in mapping.items()
            if g['required'] and (g['unproven_scopes'] or g['missing_work_scopes'])),
        'registration': registration,
        'previous': ({'revision': previous['revision'], 'sprint_digest': previous['sprint_digest'],
                      'input_digest': previous['input_digest']} if previous else None),
    }
    result['changes'] = diff(previous, result) if previous else None
    return result, spec, sources.read(LEDGER), normalized


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument('--input', required=True, type=Path)
    parser.add_argument('--authority', type=Path)
    parser.add_argument('--out', required=True, type=Path)
    parser.add_argument('--collector', default='ieo-evidence')
    parser.add_argument('--spec', default='ieo-spec')
    args = parser.parse_args()
    args.root = args.root.resolve()
    try:
        args.out.mkdir(parents=True, exist_ok=True)
        with (args.out / '.export.lock').open('a') as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            versions = sorted(args.out.glob('rev-[0-9]*'), key=lambda p: int(p.name[4:]))
            previous_path = versions[-1] if versions else None
            previous = json.loads((previous_path / 'export.json').read_bytes()) if previous_path else None
            with tempfile.TemporaryDirectory(prefix='.t09-', dir=args.out) as staging:
                staging = Path(staging)
                result, spec, state, normalized = export(args, staging, previous)
                if result is None:
                    print(previous_path)
                    return 0
                if previous:
                    require(previous['sprint_id'] == result['sprint_id'] and previous['mode'] == result['mode'],
                            'output directory belongs to another sprint or execution kind')
                target = args.out / f'rev-{result["revision"]}'
                package = staging / 'package'
                progress = package / 'progress'
                progress.mkdir(parents=True)
                (package / 'export.json').write_bytes(encode(result))
                (package / 'sprint.json').write_bytes(encode(spec))
                (package / 'input.json').write_bytes(normalized)
                (package / 'DRAFT.md').write_text(render_draft(result, spec), encoding='utf-8')
                (progress / 'state.json').write_bytes(state)
                prefix = os.path.relpath(args.root, target / 'progress') + '/'
                views = render(args.root)
                require(f"원장 SHA-256 `{sha(state)}`" in views['STATUS.md'],
                        'progress ledger changed during export; rerun against the new snapshot')
                for name, content in views.items():
                    if name.endswith('.md'):
                        content = content.replace('](../', '](' + prefix)
                    (progress / name).write_text(content, encoding='utf-8')
                package.rename(target)
                print(target)
        return 0
    except (KeyError, TypeError, ValueError, OSError, ValidationError) as error:
        print(str(error), file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
