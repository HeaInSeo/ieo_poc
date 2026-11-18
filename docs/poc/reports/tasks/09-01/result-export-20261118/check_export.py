#!/usr/bin/env python3
"""Compare an offline exporter to the frozen T09 scenarios through its CLI."""

import argparse
from copy import deepcopy
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
FIX = (HERE / 'fixtures').relative_to(ROOT)


def read(path):
    return json.loads(path.read_bytes())


def write(path, value):
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')


def project(result, spec):
    budget = result['budgets']['runtime']
    return {
        'mode': result['mode'], 'work': {k: w['state'] for k, w in result['work'].items()},
        'product_completion': result['product_completion'],
        'registration_state': spec['registration_state'],
        'run_decisions': {r['candidate']: [r['verdict'], r['adoption']] for r in result['runs']},
        'runtime_budget': {key: budget[key] for key in
                           ('attempts', 'wall_ms', 'tokens_measured', 'tokens_total', 'automatic_retry')},
        'assets': {k: a['status'] for k, a in result['assets'].items()},
        'history': {k: v for k, v in result['progress']['historical'].items() if k != 'source'},
        'denominators': {k: result['progress'][k]['total'] for k in ('main', 'followup')},
    }


def compare(args, temp):
    fixture_dir = ROOT / FIX
    expected = read(fixture_dir / 'expected.json')
    for line in (HERE / 'fixtures.sha256').read_text().splitlines():
        digest, name = line.split(maxsplit=1)
        assert digest == hashlib.sha256((fixture_dir / name).read_bytes()).hexdigest(), name
    print('PASS frozen fixture digests')

    root = temp / 'docs/poc'
    shutil.copytree(ROOT, root)
    (temp / 'contracts').mkdir()
    shutil.copyfile(ROOT.parents[1] / 'contracts/evidence-record-v0.schema.json',
                    temp / 'contracts/evidence-record-v0.schema.json')
    source_state = (root / 'progress/state.json').read_bytes()
    original = read(root / FIX / 'input.json')

    def invoke(bundle, name, rejected=None):
        inp = temp / 'input.json'
        write(inp, bundle)
        out = temp / name
        proc = subprocess.run(
            [sys.executable, str(ROOT / 'scripts/export_results.py'), '--root', str(root),
             '--input', str(inp), '--authority', str(root / FIX / 'authority.json'),
             '--out', str(out), '--collector', args.collector, '--spec', args.spec],
            capture_output=True, text=True, encoding='utf-8')
        if rejected:
            assert proc.returncode == 1 and rejected in proc.stderr, proc.stderr
            return
        assert proc.returncode == 0, proc.stderr
        output = Path(proc.stdout.strip())
        return output, read(output / 'export.json'), read(output / 'sprint.json')

    folder, baseline, spec = invoke(original, 'baseline')
    assert project(baseline, spec) == expected['baseline'], project(baseline, spec)
    assert baseline['registration']['http_status'] == 201
    assert baseline['approval'] == {'generation': 'draft', 'independent_review': 'pending',
                                    'publication': 'pending', 'execution': 'not_started'}
    assert any(r['role'] == 'development' for r in baseline['budgets']['runtime']['summary']['usage']['unmeasured'])
    assert baseline['budgets']['runtime']['summary']['by_role']['review'] == 1
    assert hashlib.sha256((folder / 'input.json').read_bytes()).hexdigest() == spec['sources'][-1]['digest']
    for name in ('STATUS.md', 'TASKS.md', 'RESULTS.md'):
        assert baseline['progress']['ledger_sha256'] in (folder / 'progress' / name).read_text()
    assert '\n## 남은 작업\n' in (folder / 'DRAFT.md').read_text()
    assert (folder / 'progress/state.json').read_bytes() == source_state
    assert invoke(original, 'baseline')[0] == folder
    print('PASS C01 baseline, collector totals, registration and idempotent package')

    for case in expected['cases']:
        bundle = deepcopy(original)
        change = case['change']
        if change == 'reverse_work':
            bundle['work'].reverse()
            assert invoke(bundle, 'reverse')[2]['digest'] == spec['digest']
            assert invoke(bundle, 'baseline')[0] == folder
        elif change == 'duplicate_record':
            bundle['runs'][2]['records'].append(bundle['runs'][0]['records'][0])
            invoke(bundle, 'duplicate', 'duplicate evidence record')
        elif change == 'cycle':
            bundle['work'][1]['after'] = ['90-03']
            invoke(bundle, 'cycle', 'DEPENDENCY_CYCLE')
        elif change == 'remove_required_work':
            bundle['work'] = [w for w in bundle['work'] if w['task_id'] != '90-03']
            _, result, draft = invoke(bundle, 'missing-work')
            assert result['goals']['partial']['missing_work_scopes'] == ['restore']
            assert draft['registration_state'] == 'draft'
        elif change == 'falsify_ledger_track':
            state = json.loads(source_state)
            main = next(t for t in state['tasks'] if t['track'] == 'main')
            follow = next(t for t in state['tasks'] if t['track'] == 'followup')
            main['track'], follow['track'] = follow['track'], main['track']
            write(root / 'progress/state.json', state)
            invoke(bundle, 'bad-track', 'ledger ID/track mismatch')
            (root / 'progress/state.json').write_bytes(source_state)
        elif change == 'zero_baseline':
            state = json.loads(source_state)
            for task in state['tasks']:
                task.update(status='미착수', latest_report=None, evidence=[], verification_level='unverified')
            write(root / 'progress/state.json', state)
            _, result, _ = invoke(bundle, 'zero')
            assert result['progress']['main'] == {'accepted': 0, 'total': 73}
            assert result['progress']['followup'] == {'accepted': 0, 'total': 12}
            assert result['progress']['historical']['total'] == 76
            (root / 'progress/state.json').write_bytes(source_state)
        elif change == 'new_target':
            bundle['goals'][0]['target']['spec_revision'] = 'fixture-spec-2'
            before = (folder / 'sprint.json').read_bytes()
            path, result, newer = invoke(bundle, 'baseline')
            assert path.name == 'rev-2' and newer['revision'] == 2
            assert set(result['work']) == set(baseline['work'])
            assert '90-01' in result['changes']['reverify_work']
            assert result['work']['90-01']['state'] == '조건 대기'
            assert (folder / 'sprint.json').read_bytes() == before
        elif change == 'revision_retry':
            entry = deepcopy(bundle['runs'][0])
            raw = read(root / entry['result'])
            raw.update(revision=2, previous_run_ref=entry['result'])
            ref = FIX / 'base-result-r2.json'
            write(root / ref, raw)
            entry.update(result=str(ref), disposition=None)
            bundle['runs'].append(entry)
            _, result, _ = invoke(bundle, 'revision')
            base_rows = [r for r in result['runs'] if r['candidate'] == 'base']
            assert [(r['revision'], r['current']) for r in base_rows] == [(1, False), (2, True)]
            assert result['budgets']['runtime']['attempts'] == 3
            assert result['budgets']['runtime']['summary']['records'] == 3
        elif change == 'mode_actual':
            bundle['mode'] = 'actual'
            _, result, draft = invoke(bundle, 'actual-kind')
            assert all(r['adoption'] == '미판정' for r in result['runs'])
            assert result['actual_linkage'] == 'pending_09-05'
            assert result['product_completion'] == 'unverified'
            assert draft['registration_state'] == 'draft'
        elif change == 'missing_prerequisite':
            bundle['work'][2]['after'] = ['99-98']
            invoke(bundle, 'missing-prerequisite', 'UNDEFINED_PREREQUISITE')
        elif change == 'evidence_recovery':
            initial, _, _ = invoke(bundle, 'recovery')
            prior = next(entry for entry in bundle['runs'] if entry['candidate'] == 'missing')
            raw = read(root / prior['result'])
            result_ref = str(FIX / 'missing-result-r2.json')
            review_ref = str(FIX / 'missing-review-r2.json')
            disposal_ref = str(FIX / 'missing-disposition-r2.json')
            raw.update(revision=2, previous_run_ref=prior['result'], operational_report_ref=review_ref)
            raw['evidence_refs'] = raw['verification']['evidence']
            raw['observed']['covered_scopes'] = ['missing']
            write(root / result_ref, raw)
            result_sha = hashlib.sha256((root / result_ref).read_bytes()).hexdigest()
            disposal = read(root / prior['disposition'])
            disposal.update(result_sha256=result_sha, decision='채택', scopes=['missing'])
            write(root / disposal_ref, disposal)
            disposal_sha = hashlib.sha256((root / disposal_ref).read_bytes()).hexdigest()
            review = read(root / FIX / 'missing-review.json')
            review.update(event_revision=2, evidence=[*raw['evidence_refs'], 'sha256:' + result_sha,
                                                       'sha256:' + disposal_sha])
            write(root / review_ref, review)
            authority_path = root / FIX / 'authority.json'
            raw_authority = authority_path.read_bytes()
            approved = json.loads(raw_authority)
            approved['approvals'][review_ref] = hashlib.sha256((root / review_ref).read_bytes()).hexdigest()
            write(authority_path, approved)
            bundle['runs'].append({**prior, 'result': result_ref, 'disposition': disposal_ref})
            bundle['work'].append({**bundle['work'][3], 'task_id': '90-08', 'purpose': 'integration',
                                   'followup': None})
            next_path, result, _ = invoke(bundle, 'recovery')
            assert next_path.name == 'rev-2' and initial.is_dir()
            assert result['work']['90-08']['state'] == '편입 대기'
            assert result['budgets']['missing']['attempts'] == 1
            assert result['goals']['missing']['unproven_scopes'] == []
            assert [(r['revision'], r['current']) for r in result['runs']
                    if r['candidate'] == 'missing'] == [(1, False), (2, True)]
            authority_path.write_bytes(raw_authority)
        elif change == 'drop_budget_history':
            bundle['runs'].pop(2)
            invoke(bundle, 'baseline', 'previous run or accounting evidence omitted')
            assert len(list((temp / 'baseline').glob('rev-*'))) == 2
        elif change == 'replace_original_evidence':
            report = read(root / bundle['runs'][0]['result'])
            evidence = root / report['evidence_refs'][0]
            original_bytes = evidence.read_bytes()
            evidence.write_bytes(original_bytes + b'\nReplaced observations.\n')
            invoke(bundle, 'baseline', 'previous result or evidence changed in place')
            assert len(list((temp / 'baseline').glob('rev-*'))) == 2
            evidence.write_bytes(original_bytes)
        elif change == 'reclassify_run_revision':
            entry = deepcopy(bundle['runs'][0])
            raw = read(root / entry['result'])
            raw.update(revision=2, previous_run_ref=entry['result'], execution_kind='actual')
            ref = FIX / 'base-result-actual-r2.json'
            write(root / ref, raw)
            entry.update(result=str(ref), disposition=None)
            bundle['runs'].append(entry)
            invoke(bundle, 'reclassified-run', 'run execution kind changed')
        elif change == 'withdraw_review':
            archived, _, _ = invoke(bundle, 'withdrawn-review')
            before = (archived / 'export.json').read_bytes()
            authority_path = root / FIX / 'authority.json'
            authority_bytes = authority_path.read_bytes()
            authority = json.loads(authority_bytes)
            report = read(root / bundle['runs'][0]['result'])
            del authority['approvals'][report['operational_report_ref']]
            write(authority_path, authority)
            next_path, result, _ = invoke(bundle, 'withdrawn-review')
            assert next_path.name == 'rev-2'
            assert result['work']['90-01']['state'] == '조건 대기'
            assert '90-01' in result['changes']['reverify_work']
            assert (archived / 'export.json').read_bytes() == before
            authority_path.write_bytes(authority_bytes)
        elif change == 'regression_without_disposition':
            bundle['work'].append({**bundle['work'][0], 'task_id': '90-08', 'purpose': 'regression',
                                   'candidate': 'alternative', 'asset_ids': []})
            _, result, _ = invoke(bundle, 'standalone-regression')
            assert result['work']['90-08']['state'] == '검토 대기'
            assert result['goals']['runtime']['verified_scopes'] == ['base']
            run = next(r for r in result['runs'] if r['candidate'] == 'alternative')
            assert run['adoption'] == '미판정'
        elif change == 'implement_rejected_candidate':
            bundle['work'].append({**bundle['work'][0], 'task_id': '90-08', 'purpose': 'implementation',
                                   'candidate': 'failure', 'asset_ids': []})
            _, result, _ = invoke(bundle, 'rejected-implementation')
            assert result['work']['90-08']['state'] == '조건 대기'
            assert result['work']['90-01']['state'] == '편입 대기'
        else:
            raise AssertionError(change)
        print(f'PASS {case["id"]} {case["expected"]}')

    assert (root / 'progress/state.json').read_bytes() == source_state
    assert (ROOT / 'progress/state.json').read_bytes() == source_state
    print('PASS shared ledger and generated views read-only')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--collector', required=True)
    parser.add_argument('--spec', required=True)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='t09-check-') as temp:
        compare(args, Path(temp))


if __name__ == '__main__':
    main()
