#!/usr/bin/env python3
"""Local draft-contract validation; does not implement authenticated registration."""
import json,copy,re
from datetime import datetime
from pathlib import Path
import jsonschema

def require_timestamp(value):
    if value is None:return
    if not isinstance(value,str) or not re.fullmatch(r'\d{4}-\d{2}-\d{2}[Tt]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:[Zz]|[+-]\d{2}:\d{2})',value):raise ValueError('timezone date-time required')
    parsed=datetime.fromisoformat(value.replace('t','T').replace('z','Z').replace('Z','+00:00'))
    if parsed.tzinfo is None:raise ValueError('timezone missing')

def validate_report(root,r):
    jsonschema.Draft202012Validator(json.loads((root/'ProgressReport_v0.schema.json').read_text()),format_checker=jsonschema.FormatChecker()).validate(r)
    typ=r['report_type'];b=r['body'];s=r['schedule'];f=s['forecast']
    for value in [r['observed_at'],r['next_check_at'],r['decision']['deadline_at'],b.get('deadline_at'),s['deadline_at']]:require_timestamp(value)
    if f['completion_range']:
        for value in f['completion_range'].values():require_timestamp(value)
    if f['state']=='estimated':
        if datetime.fromisoformat(f['completion_range']['earliest_at'])>datetime.fromisoformat(f['completion_range']['latest_at']):raise ValueError('reversed forecast')
    if s['deadline_state']=='set' and s['status']=='late' and datetime.fromisoformat(r['observed_at'])<=datetime.fromisoformat(s['deadline_at']):raise ValueError('late before deadline')
    if typ=='guardrail_review' and b['verdict']=='reject' and not b['findings']:raise ValueError('reject requires findings')
    if typ=='decision_request':
        for k in ['question','recommendation','deadline_at','default_action']:
            if r['decision'][k]!=b[k]:raise ValueError('decision mismatch '+k)
        if r['decision']['options']!=[x['label']+': '+x['impact'] for x in b['options']]:raise ValueError('decision options mismatch')
        if b['recommendation'] not in [x['label'] for x in b['options']]:raise ValueError('invalid recommendation')

def check(root):
    fixtures={p.stem:json.loads(p.read_text()) for p in (root/'reports/contract_examples').glob('*.json')}
    for r in fixtures.values():validate_report(root,r)
    human_schema=json.loads((root/'HumanSprintInput_v0.schema.json').read_text())
    hv=jsonschema.Draft202012Validator(human_schema,format_checker=jsonschema.FormatChecker())
    h={'title':'fixture','repo':'fixture','goal':'fixture','deadline':{'state':'undecided','at':None,'reason':'not yet bound'},'tasks':[{'name':'fixture','output':'fixture','done_when':'fixture','how_to_check':'fixture'}]}
    hv.validate(h)
    h['tasks'][0]['expected_duration']={'min_minutes':10,'max_minutes':20,'evidence':['fixture://past-measurement']};hv.validate(h)
    assert h['tasks'][0]['expected_duration']['min_minutes']*60==600
    bad_h=copy.deepcopy(h);bad_h['tasks'][0]['expected_duration']['evidence']=[]
    try:hv.validate(bad_h)
    except jsonschema.ValidationError:pass
    else:raise AssertionError('estimate without evidence accepted')
    cases=[]
    def bad(name,typ,change):
        r=copy.deepcopy(fixtures[typ]);change(r)
        try:validate_report(root,r)
        except (ValueError,jsonschema.ValidationError):cases.append(name);return
        raise AssertionError('accepted invalid fixture: '+name)
    bad('old actor string','development_progress',lambda r:r.update(actor='developer'))
    bad('missing contract revision','development_progress',lambda r:r.pop('contract_revision'))
    bad('developer claims verified','development_progress',lambda r:r.update(evidence_level='verified'))
    bad('wrong actor role','guardrail_review',lambda r:r['actor'].update(role='development'))
    bad('finding lacks original log','guardrail_review',lambda r:r['body']['findings'][0].pop('original_log_ref'))
    bad('reject without findings','guardrail_review',lambda r:r['body'].update(findings=[]))
    bad('estimated without measurement','coordinator_verdict',lambda r:r['schedule']['forecast'].update(state='estimated',completion_range={'earliest_at':'2026-11-13T00:00:00+09:00','latest_at':'2026-11-14T00:00:00+09:00'}))
    def reversed_forecast(r):r['schedule']['forecast'].update(state='estimated',completion_range={'earliest_at':'2026-11-14T00:00:00+09:00','latest_at':'2026-11-13T00:00:00+09:00'},evidence=['fixture://past-measurement'])
    bad('reversed forecast','coordinator_verdict',reversed_forecast)
    bad('delay without reason','coordinator_verdict',lambda r:r['schedule'].update(deadline_state='set',deadline_at='2026-11-13T00:00:00+09:00',status='at_risk'))
    bad('undecided shown normal','coordinator_verdict',lambda r:r['schedule'].update(status='on_track'))
    bad('decision mismatch','decision_request',lambda r:r['decision'].update(question='different'))
    def invalid_date(r):
        r['decision']['deadline_at']='not-a-date';r['body'].update(deadline_at='not-a-date',no_deadline_reason=None)
    bad('decision deadline is not a date','decision_request',invalid_date)
    return {'human_input_checks':{'valid_without_estimate':True,'valid_measured_estimate':True,'missing_evidence_rejected':True},'valid_role_examples':len(fixtures),'invalid_examples_rejected':len(cases),'cases':cases,'scope':'local draft contract only; runtime/human trials NOT_RUN'}
if __name__=='__main__':print(json.dumps(check(Path(__file__).resolve().parents[1]),ensure_ascii=False,indent=2))
