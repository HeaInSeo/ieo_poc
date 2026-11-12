#!/usr/bin/env python3
"""Validate v1.3.3 fixed documents and generated state, not runtime acceptance."""
import argparse,hashlib,importlib.util,json,re,sys
from pathlib import Path
from urllib.parse import unquote,urlsplit
sys.dont_write_bytecode=True
def canonical_digest(v):return hashlib.sha256(json.dumps(v,ensure_ascii=False,sort_keys=True,separators=(',',':')).encode()).hexdigest()
def graph_check(g):
    if any(len(v)!=len(set(v)) for v in g.values()):raise ValueError('duplicate dependency')
    missing=sorted({p for ds in g.values() for p in ds if p not in g})
    if missing:raise ValueError('undefined '+str(missing))
    done=set();active=set()
    def visit(k):
        if k in active:raise ValueError('cycle '+k)
        if k in done:return
        active.add(k)
        for p in g[k]:visit(p)
        active.remove(k);done.add(k)
    for k in g:visit(k)
    return {'nodes':len(g),'edges':sum(map(len,g.values())),'cycles':0,'undefined_prerequisites':0}
def ancestors(g,k):
    found=set()
    def visit(n):
        for p in g[n]:
            if p not in found:found.add(p);visit(p)
    visit(k);return found
def module(path,name):
    sp=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(sp);sp.loader.exec_module(m);return m
def validate(root):
    import jsonschema
    spec=json.loads((root/'Console_Coordinator_Sprints_v0.json').read_text());schema=json.loads((root/'SprintSpec_v0.schema.json').read_text())
    for name in ['SprintSpec_v0.schema.json','ProgressReport_v0.schema.json','HumanSprintInput_v0.schema.json']:jsonschema.Draft202012Validator.check_schema(json.loads((root/name).read_text()))
    tasks=[t for s in spec['sprints'] for t in s['tasks']];byid={t['task_id']:t for t in tasks};assert len(tasks)==len(byid)==73
    g={k:[p['task_id'] for p in t['prerequisites']] for k,t in byid.items()};out={'plan_version':'v1.3.3','main_graph':graph_check(g)}
    old=json.loads((root/'validation/v1.3.2_task_contracts.json').read_text());assert set(g)-set(old)=={'04-16'} and set(old).issubset(g)
    added=sorted((k,p) for k in old for p in set(g[k])-{d['task_id'] for d in old[k]['prerequisites']})
    removed=sorted((k,p['task_id']) for k in old for p in old[k]['prerequisites'] if p['task_id'] not in g[k])
    assert added==[('07-01','04-16')] and not removed and g['04-16']==['04-09','01-03']
    out['dependency_delta']={'existing_added':added,'removed':removed,'new_task':{'04-16':g['04-16']}}
    p4items=next(q for q in spec['inspection_profiles'] if q['revision']==4)['items']
    changed=sorted(k for k in old if old[k]['acceptance']['criteria']!=p4items[k+'.accept'])
    assert changed==sorted(['01-02','01-03','01-04','01-07','01-08','02-01','07-01','07-04','09-02','09-03'])
    for k in old:
        if k not in ['01-07','01-08']:assert byid[k]['outputs']==old[k]['outputs']
    assert '04-16' not in ancestors(g,'05-05') and '04-16' not in ancestors(g,'06-01')
    assert '04-16' in ancestors(g,'07-02') and '04-16' in ancestors(g,'07-03')
    assert '04-15' not in ancestors(g,'07-02')
    out['ancestor_counts']={k:len(ancestors(g,k)) for k in ['04-03','05-05','07-01','07-02']};out['changed_acceptance_ids']=changed
    amend=json.loads((root/'Amendment_v1.3.3-A1.json').read_text());profiles={(q['profile_id'],q['revision']):q for q in spec['inspection_profiles']}
    assert sorted(profiles)==[('poc-acceptance-v1.3.3',4),('poc-acceptance-v1.3.3',5)];p4=profiles[('poc-acceptance-v1.3.3',4)];p5=profiles[('poc-acceptance-v1.3.3',5)]
    assert sorted(k for k in p4['items'] if p4['items'][k]!=p5['items'][k])==sorted(c+'.accept' for c in amend['changed_acceptance']) and set(p4['items'])==set(p5['items'])
    revs={x['sprint_id']:x['revision'] for x in amend['sprint_revisions']};digests=[]
    for s in spec['sprints']:
        jsonschema.Draft202012Validator(schema,format_checker=jsonschema.FormatChecker()).validate(s)
        assert s['revision']==revs.get(s['sprint_id'],4) and s['contract_revision']==4 and s['registration_state']=='draft'
        assert canonical_digest({k:v for k,v in s.items() if k!='digest'})==s['digest']
        for ref in s['sources']:assert hashlib.sha256((root/ref['ref']).read_bytes()).hexdigest()==ref['digest']
        for t in s['tasks']:
            a=t['acceptance'];profile=profiles[(a['profile_id'],a['profile_revision'])]
            assert a['profile_revision']==(5 if t['task_id'] in amend['changed_acceptance'] else 4)
            assert a['criteria']==t['exit_condition']==profile['items'][a['item_ids'][0]]
            assert all(d['plan_version']=='v1.3.3' for d in t['prerequisites'])
        digests.append({'sprint_id':s['sprint_id'],'revision':s['revision'],'digest':s['digest']})
    out['sprint_digests']=digests;out['schema_errors']=0
    ledger={}
    for line in (root/'Console_Coordinator_PoC_Execution_Ledger_v1.3.3.md').read_text().splitlines():
        if re.match(r'\| \d\d-\d\d ·',line):
            cs=line.split('|');k=re.match(r' (\d\d-\d\d)',cs[1]).group(1);assert k not in ledger
            ledger[k]=re.findall(r'\d\d-\d\d',cs[3]);assert cs[4].strip()==byid[k]['acceptance']['criteria'],k
    assert ledger==g;out['ledger_spec_match']=True
    tech=json.loads((root/'Technical_Dependencies_v1.3.3.json').read_text());out['technical_graph']=graph_check({k:v['deps'] for k,v in tech.items()});assert 'P36' in tech['P26']['deps']
    f=json.loads((root/'Followup_Execution_v1.3.3.json').read_text());out['followup_graph']=graph_check({t['id']:t['deps'] for t in f['tasks']});assert len(f['tasks'])==12
    m=module(root/'scripts/render_progress.py','render');views=m.render(root)
    assert all((root/'progress'/name).read_bytes()==v.encode() for name,v in views.items());out['generated_views_match']=True
    state=json.loads((root/'progress/state.json').read_text());assert state['baselines']=={'main_tasks':73,'followup_tasks':12,'main_experiments':31,'followup_candidates':5}
    assert len(state['tasks'])==85 and len(state['experiments'])==36
    assert all(not e['runs'] for e in state['experiments']);out['runtime_probe']='NOT_RUN';out['human_trials']='NOT_RUN'
    out['progress']={'accepted_main_tasks':sum(t['status']=='검수 완료' for t in state['tasks'] if t['track']=='main'),'main_denominator':73,'followup_denominator':12}
    files=[p for p in root.glob('*.md') if not p.name.startswith(('V1_2','V1_3_','User_Decisions','PAPER_'))]
    files.append(root/'V1_3_3_Changes_and_Validation.md')
    files+=list((root/'progress').glob('*.md'))+list((root/'reports').rglob('*.md'))
    links=0
    for p in files:
        for ref in re.findall(r'!?\[[^\]]*\]\(([^)]+)\)',p.read_text()):
            ref=ref.split('#',1)[0]
            if not ref or urlsplit(ref).scheme:continue
            assert (p.parent/unquote(ref)).exists(),(str(p),ref);links+=1
    out['current_internal_links_checked']=links
    out['report_contract_checks']=module(root/'scripts/validate_contracts.py','contracts').check(root)
    sums={l[66:]:l[:64] for l in (root/'SHA256SUMS.txt').read_text().splitlines() if l.strip()};changed={c['path']:c for c in amend['files']};absent=0
    assert set(changed)<=set(sums),'amendment lists a file outside the delivered package'
    for path,orig in sums.items():
        f=root/path
        if path.startswith('progress/'):continue
        if not f.exists() and path.startswith('03_paper_repo_pairs/') and path.endswith('.pdf'):absent+=1;continue
        if path in changed:
            c=changed[path];assert c['original_sha256']==orig,path
            if c['change']=='removed_from_repository':continue
            assert hashlib.sha256(f.read_bytes()).hexdigest()==c['sha256'],path
        else:assert hashlib.sha256(f.read_bytes()).hexdigest()==orig,path
    out['package_integrity']={'delivered':len(sums),'amended':len(changed),'gitignored_paper_pdfs_absent':absent,'amendment':amend['id']}
    return out
if __name__=='__main__':
    ap=argparse.ArgumentParser();ap.add_argument('--root',type=Path,default=Path(__file__).resolve().parents[1]);ap.add_argument('--write',action='store_true');a=ap.parse_args()
    result=validate(a.root)
    if a.write:
        # v1_3_3_validation.json is the delivered (frozen) result listed in SHA256SUMS.txt and is kept as is.
        # Current results go to a separate path that is not part of the delivered package (v1.3.3-A1).
        out=a.root/'validation/v1.3.3-A1_current_validation.json'
        sums={l[66:] for l in (a.root/'SHA256SUMS.txt').read_text().splitlines() if l.strip()}
        assert out.relative_to(a.root).as_posix() not in sums,'refusing to overwrite a delivered package file'
        out.write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps(result,ensure_ascii=False,indent=2))
