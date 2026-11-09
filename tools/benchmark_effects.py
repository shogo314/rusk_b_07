#!/usr/bin/env python3
"""Compare effect-driven C++ representation changes on identical Rusk sources.

Compilation and instrumentation are excluded from timings. Optional reference
copy counts hook libstdc++'s shared_ptr increment in preprocessed GCC headers;
these counts are a development measurement, not a portable runtime interface.
"""
import argparse
import datetime
import hashlib
import json
from pathlib import Path
import platform
import statistics
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
CASES = {
    'scalar_call': ('fn add_one(x:Int)->Int{return x+1;}let i:=0;let total:=0;'
                    'while i<500000{total+=add_one(i);i+=1;}wt total;'),
    'element_call': ('fn add_one(x:Int)->Int{return x+1;}'
                     'let a:=[1;100000];let total:=0;'
                     'for i in 0..20{for x in a{total+=add_one(x);}}wt total;'),
    'find': 'let a:=[1;100000];let total:=0;for i in 0..20{total+=find(a,2)??-1;}wt total;',
    'sum_native': 'let a:=[1;1000000];let total:=0;for i in 0..20{total+=sum(a);}wt total;',
    'sum_rusk': ('fn rusk_total(a:Vec<Int>)->Int{let total:=0;for x in a{total+=x;}return total;}'
                 'let a:=[1;1000000];let total:=0;for i in 0..20{total+=rusk_total(a);}wt total;'),
}
ALLOCATION_COUNTER = r'''
#include <new>
#include <cstdio>
static unsigned long long measured_allocations=0;
void* operator new(std::size_t size){++measured_allocations;if(auto p=std::malloc(size?size:1))return p;throw std::bad_alloc();}
void operator delete(void* p)noexcept{std::free(p);}
void operator delete(void* p,std::size_t)noexcept{std::free(p);}
struct AllocationReport{~AllocationReport(){std::fprintf(stderr,"allocations=%llu\n",measured_allocations);}} allocation_report;
'''

def checked(command):
    run = subprocess.run(command, capture_output=True)
    if run.returncode:
        raise RuntimeError(f'{command}:\n{run.stderr.decode(errors="replace")}')
    return run


def build(source, executable):
    checked(['g++', '-std=c++20', '-O2', str(source), '-o', str(executable)])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiler', type=Path, default=ROOT/'target/debug/rusk')
    parser.add_argument('--baseline-compiler', type=Path, required=True)
    parser.add_argument('--repeats', type=int, default=7)
    parser.add_argument('--reference-copies', action='store_true')
    parser.add_argument('--case', action='append', choices=CASES)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.repeats < 1:
        parser.error('--repeats must be positive')
    compilers = {'baseline': args.baseline_compiler.resolve(), 'current': args.compiler.resolve()}
    report = {
        'utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'platform': platform.platform(),
        'cpu': next((line.split(':', 1)[1].strip() for line in Path('/proc/cpuinfo').read_text().splitlines() if line.startswith('model name')), platform.processor()),
        'gcc': checked(['g++', '--version']).stdout.decode().splitlines()[0],
        'compiler_sha256': {name: hashlib.sha256(path.read_bytes()).hexdigest() for name, path in compilers.items()},
        'repeats': args.repeats, 'flags': ['-std=c++20', '-O2'],
        'timing': 'interleaved; compilation excluded; instrumentation measured separately', 'cases': {},
    }
    with tempfile.TemporaryDirectory(prefix='rusk-effect-bench-') as directory:
        directory = Path(directory)
        jobs = []
        for name in args.case or CASES:
            source = directory/(name+'.rusk')
            source.write_text(CASES[name])
            row = {'source': CASES[name], 'variants': {}}
            report['cases'][name] = row
            expected = None
            for variant, compiler in compilers.items():
                stem = directory/(name+'-'+variant)
                cpp = stem.with_suffix('.cpp')
                checked([str(compiler), 'compile', str(source), '-o', str(cpp)])
                build(cpp, stem)
                output = checked([str(stem)]).stdout
                if expected is None:
                    expected = output
                assert output == expected, (name, variant, output, expected)
                data = {'output': output.decode(), 'cpp_sha256': hashlib.sha256(cpp.read_bytes()).hexdigest(), 'runs_ms': []}
                row['variants'][variant] = data
                instrumented = cpp.read_text().replace('#include <limits>', '#include <limits>\n'+ALLOCATION_COUNTER, 1)
                counted = directory/(name+'-'+variant+'-alloc.cpp')
                counted.write_text(instrumented)
                executable = counted.with_suffix('')
                build(counted, executable)
                run = checked([str(executable)])
                assert run.stdout == expected
                data['allocations'] = int(run.stderr.decode().strip().split('=')[1])
                if args.reference_copies:
                    text = checked(['g++', '-std=c++20', '-E', str(cpp)]).stdout.decode()
                    hooks = [
                        ('_M_add_ref_copy()\n      { __gnu_cxx::__atomic_add_dispatch(&_M_use_count, 1); }',
                         '_M_add_ref_copy()\n      { ++::measured_refs; __gnu_cxx::__atomic_add_dispatch(&_M_use_count, 1); }'),
                        ('_Sp_counted_base<_S_single>::_M_add_ref_copy()\n    { ++_M_use_count; }',
                         '_Sp_counted_base<_S_single>::_M_add_ref_copy()\n    { ++::measured_refs; ++_M_use_count; }'),
                    ]
                    for before, after in hooks:
                        if text.count(before) != 1:
                            raise RuntimeError('unsupported libstdc++ shared_ptr layout; omit --reference-copies')
                        text = text.replace(before, after)
                    text = 'unsigned long long measured_refs=0;\n'+text
                    text += '\nstruct RefReport{~RefReport(){std::fprintf(stderr,"copies=%llu\\n",measured_refs);}} ref_report;\n'
                    counted = directory/(name+'-'+variant+'-refs.cpp')
                    counted.write_text(text)
                    executable = counted.with_suffix('')
                    build(counted, executable)
                    run = checked([str(executable)])
                    assert run.stdout == expected
                    data['shared_ptr_copies'] = int(run.stderr.decode().strip().split('=')[1])
                jobs.append((name, variant, stem, data, expected))
            print(name, 'built and checked', flush=True)
        # Start timings only after all builds/counters complete. Reverse order
        # every other round, preserving every sample in the result artifact.
        for repeat in range(args.repeats):
            for name, variant, executable, data, expected in jobs[::1 if repeat % 2 == 0 else -1]:
                start = time.perf_counter()
                run = checked([str(executable)])
                elapsed = (time.perf_counter()-start)*1000
                assert run.stdout == expected and not run.stderr, (name, variant)
                data['runs_ms'].append(round(elapsed, 6))
        for name, row in report['cases'].items():
            for data in row['variants'].values():
                data['median_ms'] = statistics.median(data['runs_ms'])
            row['speedup'] = row['variants']['baseline']['median_ms']/row['variants']['current']['median_ms']
            print(name, row, flush=True)
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2)+'\n')


if __name__ == '__main__':
    main()
