#!/usr/bin/env python3
"""Run the reference-lifetime cases under AddressSanitizer and UBSan."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
CASES = [
    ('''struct Key{ch:Char;}
fn __eq__(a:Key,b:Key)->Bool{return a.ch==b.ch;}
let text:="abc";let values:=[Key{ch:text[0]},Key{ch:text[1]}];
let index:=values.find(Key{ch:'b'})?!;values=[];text.push_back('!');
wt index,text;wt min(3,4),max(3,4),abs(-3);
''',b'',b'1 abc!\n3 4 3\n'),
    ((ROOT/'examples/beta06/minmax.rusk').read_text(),b'',b'10 10 6\n11\n11 99\n'),
    ('''let text:="abc";let independent:=text[0]<?'z';
text.push_back('!');independent='Q';text[0]<?='A';wt text,independent;
let values:=[[8]];let old:=values[0][0];
fn replace(values:Vec<Vec<Int>>)->Int{values[0]:=[99];return 1;}
values[0][0]<?=replace(values);wt old,values;
''',b'',b'Abc! Q\n1 99\n'),
    ((ROOT/'examples/beta06/patterns.rusk').read_text(),b'',b'6\n2 3\n1\n3\ndone\n'),
    ('''struct Row{x:Int;}
let e:=Some([1]);let held:Vec<Int>:=[];
wt match e{Some(row) if {held:=row;e=None;row[0]=2;false}=>100,Some(row)=>row[0],None=>0,};
wt held;
let text:="abc";let c:=text[0];
let f:=(o:Option<Int>)=>if let Some(c)=o{c}else{0};
c:='Q';text.push_back('!');wt f(Some(3)),text;
let n:=0;
outer:while let Some(c)=if n<2{Some(text[0])}else{None}{n+=1;if n==1{continue outer;}break outer;}
text.push_back('?');wt text;
let view:="xyz";
let chosen:=match Some('a'){Some(c) if {c:=view[0];false}=>c,Some(c)=>{view.push_back('!');c}None=>'?',};wt chosen,view;
fn pick(s:Str)->Char{return match Some('a'){Some(c) if {c:=s[0];true}=>c,Some(c)=>c,None=>'?',};}
let ref:=pick(view);ref='Z';ref:='Q';view.push_back('?');wt view;
''',b'',b'2\n2\n3 abc!\nabc!?\na xyz!\nZyz!?\n'),
    ((ROOT/'examples/beta05/enums.rusk').read_text(),b'',b'5\n5\n8\n'),
    ('''enum Cell<T>{Full(T),Empty}
let text:="abc";let cell:=Cell::Full(text[0]);let alias:=cell;
let held:=match cell{Full(c)=>c,Empty=>'?',};
cell=Cell::Empty;held='Z';wt text;
held:='Q';text.push_back('!');wt held,text;
let rows:Vec<Cell<Vec<Int>>>:=[];
for i in 0..200{rows.push_back(Cell::Full([i]));}
let detached=rows;let saved:=match rows[0]{Full(r)=>r,Empty=>[],};
rows=[];saved[0]=7;wt saved,match detached[0]{Full(r)=>r,Empty=>[],};
''',b'',b'Zbc\nQ Zbc!\n7 0\n'),
    ((ROOT/'examples/beta05/blocks_and_labels.rusk').read_text(),b'',b'a b\nZbc!\n1 2\n'),
    ((ROOT/'examples/beta05/control_flow.rusk').read_text(),b'',b'b c\nab!\nab!? 2\n13\n'),
    ('''let o:=Some([1]);let alias:=o;let held:=o?!;
o=None;held[0]=7;wt alias.is_none(),held;
let d:=Dict::<Str,Vec<Int>>();d.set("a",held);let saved:=d.get("a")?!;
let detached=d;d.erase("a");saved[0]=8;wt saved,detached.get("a")?!.len();
for i in 0..1000{d.set(Str(i),[i]);}d.clear();wt saved;
''',b'',b'Yes 7\n8 1\n8\n'),
    ('''let s:=Set::<(Int,Str)>();let key:=(1,"a");s.insert(key);key[1]="b";
for k in s{k[0]=99;s.insert((1,"a"));}wt s.contains((1,"a"));
let d:=Dict::<Int,Int>();for i in 0..50{d.set(i,i);}
let it:=d.map((p:(Int,Int))=>p[1]);let other=it;
wt it.fold(0,(a:Int,b:Int)=>a+b),other.fold(0,(a:Int,b:Int)=>a+b);
d.clear();s.clear();
''',b'',b'Yes\n1225 1225\n'),
    ('''let text:="abc";let o:=Some(text[0]);let c:=if true{o?!}else{text[1]};
o=None;c='Z';wt text;let independent=c;c:='Q';text.push_back('!');wt independent,text;
in grid(2,2):Vec<Vec<Int>>,pairs(2):Vec<(Int,Str)>;wt grid,pairs;
wt try_read::<Int>().is_none();
''',b'1 2 3 4 5 x 6 y\n',b'Zbc\nZ Zbc!\n1 2 3 4 5 x 6 y\nYes\n'),
    ('''let text:="abc";let c:=text[1];let f:=()=>c;
let independent=f;c:='Q';text:="new";let held:=f();held='Z';
wt f(),independent(),text;
let values:=[1,2,3,4];let n:=0;
let it:=values.filter((x:Int)=>x%2==0).map((x:Int)=>x+(n+=1));
let copy_it=it;wt it.to_vec(),n;wt copy_it.to_vec(),n;
''',b'',b'Z b new\n3 6 2\n3 6 2\n'),
    (f'include {json.dumps(str(ROOT/"examples/beta04/library/priority_queue.rusk"))};\n'
     f'include {json.dumps(str(ROOT/"examples/beta04/library/segment_tree.rusk"))};\n'
     + '''let queue:=PriorityQueue([3,1,2],(a:Int,b:Int)=>a<b);
let detached=queue;wt queue.pop(),queue.pop(),detached.pop();
let seg:=SegTree(["a","b","c"],"",(a:Str,b:Str)=>a+b);
let changed=seg;changed.set(1,"X");wt seg.prod(0,3),changed.prod(0,3);
wt seg.max_right(0,(s:Str)=>s.len()<=2),seg.min_left(3,(s:Str)=>s.len()<=2);
''',b'',b'1 2 1\nabc aXc\n2 1\n'),
    ('''let s:="abc";
{ let c:=s[0]; c='z'; s="XYZ"; wt c; }
s.push_back('!'); wt s;
let a := [[1,2],[3]]; let row:=a[0]; let old:=row[0];
for i in 0..1000 { a.push_back([i]); }
a=[[9]]; row[0]=8; wt old,a;
let copied = a; copied[0][0]=10; wt a,copied;
''', b'', b'X\nXYZ!\n8 9\n9 10\n'),
    ('''fn first() -> Char { let s:="abc"; return s[0]; }
let saved:Vec<Char>:=[];
for i in 0..200 { saved.push_back(first()); }
for c in saved { c='Z'; }
wt saved.len(),saved[0],saved[-1];
let detached = saved; saved=[]; wt detached.len();
let s:="a";
while s[0]=='a' { s.push_back('!'); s[0]='b'; continue; }
for i in 0..3 { let c:=s[0]; if i==1 { break; } continue; }
s.push_back('?'); wt s;
''', b'', b'200 Z Z\n200\nb!?\n'),
    ('''struct Envelope<T> { payload:T; }
let text:="abc";
let envelope:=Envelope::<Char> { payload:text[0] };
let held:=envelope.payload;
envelope.payload:='Z'; held:='Q'; text.push_back('!'); wt text;
let original:=Envelope::<(Str,Vec<Int>)> { payload:("old",[1,2]) };
let old:=original.payload[1][0];
let detached = original; original=Envelope::<(Str,Vec<Int>)> { payload:("new",[9]) };
detached.payload[1][0]=7; wt old,detached.payload,original.payload;
''', b'', b'abc!\n1 old 7 2 new 9\n'),
    (f'include {json.dumps(str(ROOT/"examples/beta02/library/fenwick.rusk"))};\n'
     f'include {json.dumps(str(ROOT/"examples/beta02/library/sum_segment_tree.rusk"))};\n'
     + '''let fw:=Fenwick([1,2,3],0); let detached = fw;
fw.add(1,4); wt fw.prefix_sum(3),detached.prefix_sum(3);
let seg:=SumSegTree(["a","b","c"],""); seg.set(1,"X"); wt seg.prod(0,3);
''', b'', b'10 6\naXc\n'),
    ('''fn fresh(x:Int)->Int { return x*2+1; }
let saved:Vec<Int>:=[];
for i in 0..1000 { saved.push_back(fresh(i)); }
let held:=saved[0]; let other = saved; held=8;
wt saved[0],other[0],saved[-1];
in n:Int,s:Str; wt n,s.len(),s[0],s[-1];
s.push_back('!'); wt s.len();
''', b'7 '+b'x'*131100+b'\n', b'8 1 1999\n7 131100 x x\n131101\n'),
    ('''fixed let s:="abc";
{fixed let c:=s[0];fixed let f:=()=>{c='Z';c};wt f();}
s.push_back('!');wt s;
for i in 0..3 {
 fixed let c:=s[i];c='A';
 if i==1{continue;}
 if i==2{break;}
}
s.push_back('?');wt s;
''', b'', b'Z\nZbc!\nAAA!?\n'),
    ('''let text:="abc";
{
 readonly fixed let view:=text;
 wt view[0],view.len();
 let detached=view[0];detached='Q';wt detached;
 let reader:=()=>view[0];wt reader();
 text[0]='Z';wt reader();
}
text.push_back('!');wt text;
''', b'', b'a 3\nQ\na\nZ\nZbc!\n'),
    ('''fn bump(noescape c:Char){c='Z';}
fn char_copy(readonly noescape text:Str)->Char{return copy(text[0]);}
fn run<F>(noescape callback:F){callback();}
let s:="abc";
{let c:=s[0];bump(c);run(()=>c);wt char_copy(s),s;}
s.push_back('!');wt s;
''', b'', b'Z Zbc\nZbc!\n'),
    ('''fn positive(x:Int)->Bool{let flag:=x>0;let alias:=flag;return alias && true;}
let total:=0;
for i in 0..5{let flag:=i>0;let alias:=flag;if alias && positive(i){total+=1;}}
let s:="abc";
fn code(c:Char)->Int{return Int(c);}
wt total,code(s[0]);s.push_back('!');wt s;
struct Box{value:Int;}
readonly let r:=1;let a:=Box{value:0};let b:=Box{value:0};a.value:=r;b.value=2;wt a.value,b.value;
''', b'', b'4 97\nabc!\n1 2\n'),
    ('''let total:=0;let i:=0;
while i<100{let value:=i+1;let alias:=value;
let c:='a';let ca:=c;if ca=='a'{total+=alias;}i+=1;}
wt total;
let s:="abc";{let c:=s[0];let alias:=c;alias='Z';wt s;}
s.push_back('!');wt s;
''', b'', b'5050\nZbc\nZbc!\n'),
    ('''struct Box{value:Int;}
fn update(noescape x:Int){let b:=Box{value:0};let alias:=b;alias.value:=x;b.value+=1;}
struct Letter{value:Char;}
fn scoped(noescape x:Char)->Int{let b:=Letter{value:'?'};b.value:=x;wt b.value;return 1;}
let x:=2;update(x);wt x;
let text:="abc";wt scoped(text[0]);text.push_back('!');wt text;
''', b'', b'3\na\n1\nabc!\n'),
    ('''struct Letter{value:Char;}
fn nested(noescape x:Char)->Int{
 let child:=Letter{value:'?'};let outer:=(child,0);let alias:=outer[0];
 alias.value:=x;wt outer[0].value;return 1;
}
struct Cell{value:Int;}
fn repeated(noescape x:Int)->Int{
 let outer:=(Cell{value:0},0);let total:=0;
 for i in 0..3{let old:=outer[0];outer[0]:=Cell{value:1};old.value:=x;total+=old.value;}
 return total;
}
let text:="abc";wt nested(text[0]);text.push_back('!');wt text,repeated(3);
''', b'', b'a\n1\nabc! 9\n'),
    ('''struct Letter{value:Char;}
struct Holder{letter:Letter;}
fn identity(h:Holder)->Holder{return h;}
fn child(h:Holder)->Letter{return h.letter;}
fn scoped(noescape x:Char)->Int{
 let h:=Holder{letter:Letter{value:'?'}};let alias:=child(identity(h));
 alias.value:=x;wt h.letter.value;return 1;
}
let text:="abc";wt scoped(text[0]);text.push_back('!');wt text;
''', b'', b'a\n1\nabc!\n'),
    ('''struct Letter{value:Char;}
struct Holder{letter:Letter;}
fn pack(c:Letter)->(Holder,Int){return (Holder{letter:c},0);}
fn scoped(noescape x:Char)->Int{
 let c:=Letter{value:'?'};let held:=pack(c);
 c.value:=x;wt held[0].letter.value;return 1;
}
let text:="abc";wt scoped(text[0]);text.push_back('!');wt text;
''', b'', b'a\n1\nabc!\n'),
    ('''let values:=[1,2,3];let total:=0;
for x in values{let alias:=x;let other:=alias;total+=other;}wt total;
let text:="ab";let chars:=[text[0]];
{let c:=chars[0];let alias:=c;wt alias;}
chars=[];text.push_back('!');wt text;
''', b'', b'6\na\nab!\n'),
    ('''fn total(n:Int)->Int{
 let value:=0;let alias:=value;for i in 0..n{alias+=i;}return value;
}
let result:=total(100);let saved:=result;wt result;result=7;wt saved,total(5);
''', b'', b'4950\n7 10\n'),
    ('''let values:=[1,2,3];let total:=0;
for x in values{let alias:=x;total+=alias;}wt total;
let numbers:=[1];let other:=numbers[0];for x in numbers{other=8;wt x;}
let text:="ab";let chars:=[text[0],text[1]];
for ch in chars{let alias:=ch;wt alias;}chars=[];text.push_back('!');wt text;
let held:={let a:=[7];a[0]};wt held;
let a:=[1];for x in a{a[0]:=9;wt x;}wt a;
extern fn deferred(a:Vec<Int>)->Int{
 cpp_name:"deferred",
 code:"Obj<std::int64_t> deferred(Obj<IntVec> a,Loc){return Obj<std::int64_t>(new std::int64_t(0),[a](std::int64_t* p){(*a)[0]=make_obj<std::int64_t>(9);delete p;});}",
}
let b:=[1];{let token:=deferred(b);for x in b{{let alias:=token;}wt x;}}wt b;
''', b'', b'6\n8\na\nb\nab!\n7\n1\n9\n1\n9\n'),
    ('''fn next(x:Int)->Int{return x+1;}
let a:=[1,2,3];let total:=0;for x in a{let alias:=x;total+=next(alias);}wt total;
fn code(c:Char)->Bool{return c=='a';}
let text:="ab";let chars:=[text[0],text[1]];
for ch in chars{wt code(ch);}chars=[];text.push_back('!');wt text;
fn identity(x:Int)->Int{return x;}
let b:=[4];for x in b{let saved:=identity(x);saved=7;wt x;}wt b;
''', b'', b'9\nYes\nNo\nab!\n7\n7\n'),
    ('''fn across(x:Int,n:Int)->Int{
 let alias:=x;let total:=0;
 for i in 0..n{let local:=alias;if i%2==0{total+=local;}else{total+=x+1;}}
 return total;
}
fn recur(x:Int)->Int{if x==0{return 0;}return x+recur(x-1);}
wt across(3,3),recur(5);
let value:=4;wt across(value,{value:=9;2}),value;
fn code(c:Char)->Bool{let alias:=c;return alias=='a';}
let text:="ab";wt code(text[0]);text.push_back('!');wt text;
fn rebound(x:Int)->Int{let saved:=x;x:=7;return saved+x;}wt rebound(2);
''', b'', b'10 15\n9 9\nYes\nab!\n9\n'),
    ('''let a:=[1,2,3];let total:=0;
for x in a{let alias:=x;if x%2==0{total+=alias;}else{total+=x+1;}}wt total;
let b:=[4];let old:=b[0];let alias:=old;if true{b[0]:=9;}else{b=[];}wt alias,b;
fn take(text:Str,flag:Bool){let chars:=[text[0]];let c:=chars[0];let alias:=c;if flag{wt alias;return;}wt c;}
let text:="ab";take(text,true);text.push_back('!');take(text,false);text.push_back('?');wt text;
fn run(flag:Bool){let text:="ab";let chars:=[text[0]];
 {let c:=chars[0];let alias:=c;if flag{chars=[];}else{chars[0]:='z';chars=[];}wt alias;}
 text.push_back('!');wt text;
}
run(true);run(false);
''', b'', b'8\n4 9\na\na\nab!?\na\nab!\na\nab!\n'),
    ((ROOT/'examples/prototype/grid_bfs.rusk').read_text(), b'3 4\n....\n.##.\n....\n', b'5\n'),
]

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--no-leaks', action='store_true', help='disable LeakSanitizer for ptrace environments')
    args=parser.parse_args()
    env=os.environ.copy()
    if args.no_leaks:
        env['ASAN_OPTIONS']=env.get('ASAN_OPTIONS','')+':detect_leaks=0'
    with tempfile.TemporaryDirectory(prefix='rusk-sanitizers-') as directory:
        temp=Path(directory)
        for source, data, expected in CASES:
            (temp/'main.rusk').write_text(source)
            subprocess.run([str(ROOT/'target/debug/rusk'),'compile',str(temp/'main.rusk'),'-o',str(temp/'main.cpp')],check=True)
            subprocess.run(['g++','-std=c++20','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer',str(temp/'main.cpp'),'-o',str(temp/'main')],check=True)
            run=subprocess.run([str(temp/'main')],input=data,capture_output=True,env=env)
            assert run.returncode == 0, run.stderr.decode(errors="replace")
            assert run.stdout==expected, (run.stdout,expected)
            assert not run.stderr, run.stderr
    print(f'AddressSanitizer / UBSan: {len(CASES)} cases passed')

if __name__ == '__main__':
    main()
