#!/usr/bin/env python3
"""Compare generated C++, handwritten C++, and PyPy; compilation is not timed."""
import argparse
import datetime
import json
import hashlib
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
RANGE_RUSK = 'in n:Int; let total:=0; for i in 0..n { total+=i; } wt total;'
RANGE_CPP = '#include <iostream>\nint main(){long long n,total=0;std::cin>>n;for(long long i=0;i<n;++i)total+=i;std::cout<<total<<"\\n";}'
RANGE_PY = 'n=int(input()); total=0\nfor i in range(n): total+=i\nprint(total)\n'
SEARCH_CPP = '''#include <bits/stdc++.h>
int main(){std::ios::sync_with_stdio(false);std::cin.tie(nullptr);int n,q;std::cin>>n>>q;std::vector<long long>a(n);for(auto& x:a)std::cin>>x;std::stable_sort(a.begin(),a.end());while(q--){long long x;std::cin>>x;std::cout<<(std::upper_bound(a.begin(),a.end(),x)-a.begin())<<'\\n';}}'''
SEARCH_PY = '''import sys,bisect
it=iter(map(int,sys.stdin.buffer.read().split()));n=next(it);q=next(it)
a=sorted(next(it) for _ in range(n))
sys.stdout.write(''.join(str(bisect.bisect_right(a,next(it)))+'\\n' for _ in range(q)))
'''
GRID_CPP = '''#include <bits/stdc++.h>
int main(){int h,w;std::cin>>h>>w;std::vector<std::string>g(h);for(auto& r:g)std::cin>>r;std::vector<std::vector<int>>d(h,std::vector<int>(w,-1));std::queue<std::pair<int,int>>q;if(g[0][0]!='#'){d[0][0]=0;q.push({0,0});}int dr[]={1,-1,0,0},dc[]={0,0,1,-1};while(!q.empty()){auto[r,c]=q.front();q.pop();for(int i=0;i<4;i++){int nr=r+dr[i],nc=c+dc[i];if(nr>=0&&nr<h&&nc>=0&&nc<w&&g[nr][nc]!='#'&&d[nr][nc]<0){d[nr][nc]=d[r][c]+1;q.push({nr,nc});}}}std::cout<<d[h-1][w-1]<<'\\n';}'''
GRID_PY = '''import sys
from collections import deque
h,w=map(int,sys.stdin.buffer.readline().split());g=[sys.stdin.buffer.readline().strip() for _ in range(h)];d=[[-1]*w for _ in range(h)];q=deque()
if g[0][0]!=35:d[0][0]=0;q.append((0,0))
while q:
 r,c=q.popleft()
 for dr,dc in ((1,0),(-1,0),(0,1),(0,-1)):
  nr,nc=r+dr,c+dc
  if 0<=nr<h and 0<=nc<w and g[nr][nc]!=35 and d[nr][nc]<0:d[nr][nc]=d[r][c]+1;q.append((nr,nc))
print(d[h-1][w-1])
'''


INPUT_RUSK = 'in n:Int; let total:=0; for i in 0..n { in x:Int; total+=x; } wt total;'
INPUT_CPP = '#include <iostream>\nint main(){std::ios::sync_with_stdio(false);std::cin.tie(nullptr);int n;std::cin>>n;long long total=0,x;while(n--){std::cin>>x;total+=x;}std::cout<<total<<"\\n";}'
INPUT_PY = 'import sys\nit=iter(map(int,sys.stdin.buffer.read().split())); n=next(it); print(sum(it))\n'
OUTPUT_RUSK = 'in n:Int; for i in 0..n { wt i; }'
OUTPUT_CPP = '#include <iostream>\nint main(){std::ios::sync_with_stdio(false);std::cin.tie(nullptr);int n;std::cin>>n;for(int i=0;i<n;++i)std::cout<<i<<"\\n";}'
OUTPUT_PY = 'import sys\nn=int(input());sys.stdout.write("".join(str(i)+"\\n" for i in range(n)))\n'
FENWICK_RUSK = 'include "'+str(ROOT/'examples/beta02/library/fenwick.rusk')+'"; in n,q:Int; let fw:=Fenwick(n,0); for i in 0..q { in op,x,y:Int; if op==0 { fw.add(x,y); } else { wt fw.range_sum(x,y); } }'
FENWICK_CPP = '#include <bits/stdc++.h>\nint main(){std::ios::sync_with_stdio(false);std::cin.tie(nullptr);int n,q;std::cin>>n>>q;std::vector<long long>t(n+1);auto sum=[&](int i){long long s=0;for(;i;i-=i&-i)s+=t[i];return s;};while(q--){int op,x,y;std::cin>>op>>x>>y;if(op==0){for(int i=x+1;i<=n;i+=i&-i)t[i]+=y;}else std::cout<<sum(y)-sum(x)<<"\\n";}}'
FENWICK_PY = '''import sys
it=iter(map(int,sys.stdin.buffer.read().split()));n=next(it);q=next(it);t=[0]*(n+1);out=[]
def prefix(i):
 s=0
 while i:s+=t[i];i-=i&-i
 return s
for _ in range(q):
 op=next(it);x=next(it);y=next(it)
 if op==0:
  i=x+1
  while i<=n:t[i]+=y;i+=i&-i
 else:out.append(str(prefix(y)-prefix(x)))
sys.stdout.write('\\n'.join(out)+'\\n')
'''

CALLBACK_FOLD_RUSK = 'in n:Int; wt fold((0..n).map((x:Int)=>x*2).filter((x:Int)=>x%3==0),0,(a:Int,b:Int)=>a+b);'
CALLBACK_FOLD_MANUAL = 'in n:Int; let total:=0;for i in 0..n {let value:=i*2;if value%3==0{total+=value;}}wt total;'
CALLBACK_FOLD_CPP = '#include <iostream>\nint main(){long long n,total=0;std::cin>>n;for(long long i=0;i<n;++i){auto x=i*2;if(x%3==0)total+=x;}std::cout<<total<<"\\n";}'
CALLBACK_FOLD_PY = 'n=int(input());total=0\nfor i in range(n):\n x=i*2\n if x%3==0:total+=x\nprint(total)\n'
CALLBACK_SORT_RUSK = 'struct Item{key:Int;id:Int;}in n:Int;let values:Vec<Item>:=[];for i in 0..n{in key:Int;values.push_back(Item{key:key,id:copy(i)});}values.sort_by_key((x:Item)=>x.key);for x in values{wt x.key,x.id;}'
CALLBACK_SORT_MANUAL = 'in n:Int;let values:Vec<(Int,Int)>:=[];for i in 0..n{in key:Int;values.push_back((key,copy(i)));}values.sort();for x in values{wt x[0],x[1];}'
CALLBACK_SORT_CPP = '#include <bits/stdc++.h>\nint main(){std::ios::sync_with_stdio(false);std::cin.tie(nullptr);int n;std::cin>>n;std::vector<std::pair<long long,long long>> a(n);for(int i=0;i<n;++i){std::cin>>a[i].first;a[i].second=i;}std::stable_sort(a.begin(),a.end(),[](auto x,auto y){return x.first<y.first;});for(auto [key,id]:a)std::cout<<key<<" "<<id<<"\\n";}'
CALLBACK_SORT_PY = 'import sys\nit=iter(map(int,sys.stdin.buffer.read().split()));n=next(it);a=sorted(((next(it),i) for i in range(n)),key=lambda x:x[0]);sys.stdout.write("".join(str(k)+" "+str(i)+"\\n" for k,i in a))\n'
MANUAL_RUSK = {'callback_fold':CALLBACK_FOLD_MANUAL,'callback_sort':CALLBACK_SORT_MANUAL}

DICTIONARY_RUSK = 'in n:Int;let d:=Dict::<Int,Int>();for i in 0..n{in key:Int;d[key]+=1;}let total:=0;for (key,value) in d{total+=key*value;}wt total,d.len();'
DICTIONARY_CPP = '#include <bits/stdc++.h>\nint main(){std::ios::sync_with_stdio(false);std::cin.tie(nullptr);int n;std::cin>>n;std::unordered_map<long long,long long>d;while(n--){long long k;std::cin>>k;++d[k];}long long total=0;for(auto[k,v]:d)total+=k*v;std::cout<<total<<" "<<d.size()<<"\\n";}'
DICTIONARY_PY = 'import sys\nit=iter(map(int,sys.stdin.buffer.read().split()));n=next(it);d={}\nfor k in it:d[k]=d.get(k,0)+1\nprint(sum(k*v for k,v in d.items()),len(d))\n'
SET_RUSK = 'in n:Int;let s:=Set::<(Int,Int)>();for i in 0..n{in p:(Int,Int);s.insert(p);}let total:=0;for (a,b) in s{total+=a+b;}wt total,s.len();'
SET_CPP = '#include <bits/stdc++.h>\nstruct Hash{size_t operator()(const std::pair<long long,long long>&p)const{return std::hash<long long>{}(p.first)^(std::hash<long long>{}(p.second)+0x9e3779b9+(std::hash<long long>{}(p.first)<<6)+(std::hash<long long>{}(p.first)>>2));}};int main(){std::ios::sync_with_stdio(false);std::cin.tie(nullptr);int n;std::cin>>n;std::unordered_set<std::pair<long long,long long>,Hash>s;while(n--){long long a,b;std::cin>>a>>b;s.insert({a,b});}long long total=0;for(auto[a,b]:s)total+=a+b;std::cout<<total<<" "<<s.size()<<"\\n";}'
SET_PY = 'import sys\nit=iter(map(int,sys.stdin.buffer.read().split()));n=next(it);s={(next(it),next(it)) for _ in range(n)};print(sum(a+b for a,b in s),len(s))\n'
OPTION_RUSK = 'in n:Int;let total:=0;for i in 0..n{let value:=if i%3==0{None::<Int>()}else{Some(i)};total+=value??7;}wt total;'
OPTION_CPP = '#include <iostream>\nint main(){long long n,total=0;std::cin>>n;for(long long i=0;i<n;++i)total+=(i%3==0?7:i);std::cout<<total<<"\\n";}'
OPTION_PY = 'n=int(input());total=0\nfor i in range(n):total+=7 if i%3==0 else i\nprint(total)\n'
BETA05_CASES = {'dictionary', 'tuple_set', 'option'}

def version(command):
    result = subprocess.run(command, capture_output=True, text=True, check=True)
    return (result.stdout or result.stderr).strip().splitlines()[0]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiler', type=Path, default=ROOT/'target/debug/rusk')
    parser.add_argument('--baseline-compiler', type=Path, help='compare a previous Rusk compiler on the same inputs')
    parser.add_argument('--baseline-all-cases', action='store_true', help='baseline supports every selected case, including beta04 callbacks and beta05 containers')
    parser.add_argument('--compare-no-opt', action='store_true', help='also check and measure the same compiler with --no-opt')
    parser.add_argument('--repeats', type=int, default=3)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--case', action='append', choices=['range_sum','integer_input','integer_output','sort_search','grid_bfs','fenwick','callback_fold','callback_sort','dictionary','tuple_set','option'], help='run only selected cases (repeatable)')
    args = parser.parse_args()
    if args.baseline_all_cases and not args.baseline_compiler:
        parser.error('--baseline-all-cases requires --baseline-compiler')
    if args.repeats < 1:
        parser.error('--repeats must be positive')
    pypy = shutil.which('pypy3')
    n, q, h, w = 200_000, 2_000, 200, 200
    values = [(i*7919)%1_000_003 for i in range(n)]
    queries = [(i*12347)%1_000_003 for i in range(q)]
    integer_data = ('1000000\n'+' '.join(str(i%1000) for i in range(1_000_000))+'\n').encode()
    fenwick_n, fenwick_q = 100_000, 100_000
    operations = [f"0 {(i*7919)%fenwick_n} {i%100}" if i%2==0 else f"1 0 {(i*12347)%fenwick_n}" for i in range(fenwick_q)]
    fenwick_data = (f"{fenwick_n} {fenwick_q}\n"+'\n'.join(operations)+'\n').encode()
    sort_data = ('100000\n'+' '.join(str((i*7919)%97) for i in range(100_000))+'\n').encode()
    dictionary_data=('200000\n'+' '.join(str((i*7919)%10000) for i in range(200000))+'\n').encode()
    set_data=('200000\n'+'\n'.join(f'{i%10000} {(i*7919)%10000}' for i in range(200000))+'\n').encode()
    jobs = [
        ('option', OPTION_RUSK, OPTION_CPP, OPTION_PY, b'1000000\n', 'N=1,000,000; Some/None and ??'),
        ('dictionary', DICTIONARY_RUSK, DICTIONARY_CPP, DICTIONARY_PY, dictionary_data, 'N=200,000; 10,000 distinct keys'),
        ('tuple_set', SET_RUSK, SET_CPP, SET_PY, set_data, 'N=200,000; 10,000 distinct pairs'),
        ('callback_fold', CALLBACK_FOLD_RUSK, CALLBACK_FOLD_CPP, CALLBACK_FOLD_PY, b'1000000\n', 'N=1,000,000'),
        ('callback_sort', CALLBACK_SORT_RUSK, CALLBACK_SORT_CPP, CALLBACK_SORT_PY, sort_data, 'N=100,000'),
        ('integer_input', INPUT_RUSK, INPUT_CPP, INPUT_PY, integer_data, 'N=1,000,000'),
        ('integer_output', OUTPUT_RUSK, OUTPUT_CPP, OUTPUT_PY, b'200000\n', 'N=200,000'),
        ('fenwick', FENWICK_RUSK, FENWICK_CPP, FENWICK_PY, fenwick_data, 'N=100,000; Q=100,000'),
        ('range_sum', RANGE_RUSK, RANGE_CPP, RANGE_PY, b'1000000\n', 'N=1,000,000'),
        ('sort_search', (ROOT/'examples/prototype/sort_search.rusk').read_text(), SEARCH_CPP, SEARCH_PY,
         (f'{n} {q}\n'+' '.join(map(str,values))+'\n'+' '.join(map(str,queries))+'\n').encode(), 'N=200,000; Q=2,000'),
        ('grid_bfs', (ROOT/'examples/prototype/grid_bfs.rusk').read_text(), GRID_CPP, GRID_PY,
         (f'{h} {w}\n'+('.'*w+'\n')*h).encode(), '200×200 open grid'),
    ]
    if args.case:
        jobs = [job for job in jobs if job[0] in args.case]
    results = []
    with tempfile.TemporaryDirectory(prefix='rusk-benchmark-') as directory:
        temp = Path(directory)
        for name, rusk, cpp, python, data, size in jobs:
            (temp/'main.rusk').write_text(rusk)
            subprocess.run([str(args.compiler.resolve()),'compile',str(temp/'main.rusk'),'-o',str(temp/'rusk.cpp')],check=True)
            (temp/'native.cpp').write_text(cpp)
            (temp/'baseline.py').write_text(python)
            for label in ['rusk','native']:
                subprocess.run(['g++','-std=c++20','-O2',str(temp/f'{label}.cpp'),'-o',str(temp/label)],check=True)
            commands = {'C++':[str(temp/'native')], 'Rusk':[str(temp/'rusk')]}
            if args.compare_no_opt:
                subprocess.run([str(args.compiler.resolve()),'compile',str(temp/'main.rusk'),'--no-opt','-o',str(temp/'plain.cpp')],check=True)
                subprocess.run(['g++','-std=c++20','-O2',str(temp/'plain.cpp'),'-o',str(temp/'plain')],check=True)
                commands['Rusk no-opt']=[str(temp/'plain')]
            if name in MANUAL_RUSK:
                (temp/'manual.rusk').write_text(MANUAL_RUSK[name])
                subprocess.run([str(args.compiler.resolve()),'compile',str(temp/'manual.rusk'),'-o',str(temp/'manual.cpp')],check=True)
                subprocess.run(['g++','-std=c++20','-O2',str(temp/'manual.cpp'),'-o',str(temp/'manual')],check=True)
                commands['Rusk manual']=[str(temp/'manual')]
            if args.baseline_compiler and (args.baseline_all_cases or (name not in MANUAL_RUSK and name not in BETA05_CASES)):
                subprocess.run([str(args.baseline_compiler.resolve()),'compile',str(temp/'main.rusk'),'-o',str(temp/'previous.cpp')],check=True)
                subprocess.run(['g++','-std=c++20','-O2',str(temp/'previous.cpp'),'-o',str(temp/'previous')],check=True)
                commands['Rusk baseline']=[str(temp/'previous')]
            if pypy:
                commands['PyPy']=[pypy,str(temp/'baseline.py')]
            reference = None
            row = {'name':name, 'size':size, 'input_sha256':hashlib.sha256(data).hexdigest(), 'source_sha256':hashlib.sha256(rusk.encode()).hexdigest(), 'seconds':{}, 'samples':{}}
            samples = {label: [] for label in commands}
            # Interleave implementations to reduce systematic ordering bias.
            for repeat in range(args.repeats):
                labels = list(commands)
                labels = labels[repeat % len(labels):] + labels[:repeat % len(labels)]
                for label in labels:
                    start=time.perf_counter()
                    run=subprocess.run(commands[label],input=data,capture_output=True,check=True)
                    samples[label].append(time.perf_counter()-start)
                    if reference is None:
                        reference=run.stdout
                    if run.stdout != reference:
                        raise RuntimeError(f'{name}: output mismatch in {label}')
            for label in commands:
                row['seconds'][label]=statistics.median(samples[label])
                row['samples'][label]=samples[label]
            if 'Rusk baseline' in row['seconds']:
                row['speedup_vs_baseline']=row['seconds']['Rusk baseline']/row['seconds']['Rusk']
            results.append(row)
    report = {'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(), 'platform':platform.platform(),
              'cpu':next((line.split(':',1)[1].strip() for line in Path('/proc/cpuinfo').read_text().splitlines() if line.startswith('model name')), 'unknown'),
              'g++':version(['g++','--version']), 'pypy':version([pypy,'--version']) if pypy else None,
              'compiler_sha256':hashlib.sha256(args.compiler.read_bytes()).hexdigest(),
              'baseline_compiler_sha256':hashlib.sha256(args.baseline_compiler.read_bytes()).hexdigest() if args.baseline_compiler else None,
              'repeats':args.repeats, 'results':results}
    output=json.dumps(report,ensure_ascii=False,indent=2)+'\n'
    print(output,end='')
    if args.output:
        args.output.write_text(output)

if __name__ == '__main__':
    main()
